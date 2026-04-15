//! Heightmap terrain renderer — Whitted raytracer over a heightfield.
//!
//! Rendering pipeline per pixel:
//!   1. Cast a pinhole camera ray.
//!   2. Sphere-step raymarch against the heightfield until hit or escape.
//!   3. On **hit**: Whitted shading —
//!        a. Finite-difference surface normal.
//!        b. Shadow ray: march toward the sun; hard shadow if terrain is hit.
//!        c. Albedo from height / slope (dirt → rock → snow).
//!        d. Lambert diffuse + sky ambient (attenuated by normal-up component).
//!        e. Fresnel-weighted specular reflection ray (Whitted bounce, depth ≤ 1).
//!        f. Reinhard tone-mapping.
//!   4. On **miss**: sky gradient + sun disc.

use std::env;
use std::sync::mpsc;
use std::time::Instant;

use image::{ImageError, RgbImage};
use rayon::prelude::*;

use crate::{
    EvalState, GraphCamera, GraphRenderSourceKind, GraphSceneSettings, GraphSky, GraphSun,
    ObjectValue, Value, compile_specialized_height_node_eval_function,
    compile_specialized_shell_node_eval_function, eval_node_function,
    render_api::{Camera, PinholeCamera, Spectrum, Vec3},
};

use super::height_source::HeightSampler;
use super::wgpu_field::{GpuFieldRaster, try_rasterize_field_wgpu};
use super::{RenderError, RenderProgress};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MaterialDebugMode {
    Off,
    Mask,
    Lanes,
}

impl MaterialDebugMode {
    fn from_cli(value: Option<&str>) -> Self {
        match value {
            Some("mask") => Self::Mask,
            Some("lanes") => Self::Lanes,
            _ => Self::Off,
        }
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Render a terrain scene to an RGB image using the Whitted raytracer.
pub fn render_terrain_png<F>(
    state: &EvalState,
    source_kind: GraphRenderSourceKind,
    width: u32,
    height: u32,
    tile_size: u32,
    camera: &GraphCamera,
    sun: &GraphSun,
    sky: &GraphSky,
    scene: &GraphSceneSettings,
    material_preview: bool,
    material_debug: Option<&str>,
    mut progress_cb: F,
) -> Result<RgbImage, RenderError>
where
    F: FnMut(RenderProgress, &RgbImage) -> Result<(), ImageError> + Send,
{
    let require_gpu_field = matches!(
        env::var("FORGEDTHOUGHTS_REQUIRE_GPU_FIELD").ok().as_deref(),
        Some("1" | "true" | "TRUE" | "yes" | "YES" | "on" | "ON")
    );
    // Find the graph binding.
    let graph_binding = state
        .bindings
        .get("graph")
        .ok_or(RenderError::MissingGraph)?;
    let graph_obj = match &graph_binding.value {
        Value::Object(obj) => obj.clone(),
        _ => return Err(RenderError::MissingGraph),
    };
    let material_lanes = TerrainMaterialLanes::from_state(state);
    let material_debug_mode = MaterialDebugMode::from_cli(material_debug);
    let trace_material_distribution = matches!(
        env::var("FORGEDTHOUGHTS_TRACE_MATERIAL_DISTRIBUTION")
            .ok()
            .as_deref(),
        Some("1" | "true" | "TRUE" | "yes" | "YES" | "on" | "ON")
    );
    if material_preview {
        if trace_material_distribution && let Some(lanes) = material_lanes.as_ref() {
            report_material_distribution(lanes, scene.world_size.max(f32::EPSILON));
        }
        return render_material_preview_sphere(
            width,
            height,
            tile_size,
            sun,
            sky,
            scene,
            material_lanes.as_ref(),
            material_debug_mode,
            progress_cb,
        );
    }
    if trace_material_distribution && let Some(lanes) = material_lanes.as_ref() {
        report_material_distribution(lanes, scene.world_size.max(f32::EPSILON));
    }

    let mut gpu_field_raster: Option<GpuFieldRaster> = None;
    if matches!(source_kind, GraphRenderSourceKind::FieldScalar) {
        let raster_width = width.max(1).saturating_mul(2).min(4096);
        let raster_height = height.max(1).saturating_mul(2).min(4096);
        let raster_settings = super::node::NodeRenderSettings {
            width: raster_width,
            height: raster_height,
            tile_size,
            world_size: scene.world_size.max(f32::EPSILON),
        };
        match try_rasterize_field_wgpu(state, source_kind, &graph_obj, raster_settings) {
            Ok(Some(raster)) => gpu_field_raster = Some(raster),
            Ok(None) => {
                if require_gpu_field {
                    return Err(RenderError::Backend(
                        "GPU field backend was required but this terrain graph could not be translated to GPU".to_string(),
                    ));
                }
            }
            Err(err) => {
                if require_gpu_field {
                    return Err(err);
                }
            }
        }
    }

    let sampler = HeightSampler::new(state, graph_obj.clone(), source_kind);
    let node_name = graph_obj
        .type_name
        .clone()
        .ok_or(RenderError::MissingGraph)?;

    // Try to compile a JIT-specialised eval kernel.
    let compiled_eval = matches!(source_kind, GraphRenderSourceKind::PointScalar)
        .then(|| compile_specialized_height_node_eval_function(state, &node_name, Some(&graph_obj)))
        .flatten();

    let width = width.max(1);
    let height = height.max(1);
    let tile_size = tile_size.max(8);
    let aspect = width as f32 / height as f32;

    // Build PinholeCamera from graph settings.
    let pinhole = PinholeCamera {
        origin: Vec3::new(camera.origin[0], camera.origin[1], camera.origin[2]),
        target: Vec3::new(camera.target[0], camera.target[1], camera.target[2]),
        up: Vec3::new(camera.up[0], camera.up[1], camera.up[2]),
        fov_y_degrees: camera.fov_y,
    };

    // Normalise sun direction once.
    let sd = sun.direction;
    let sd_len = (sd[0] * sd[0] + sd[1] * sd[1] + sd[2] * sd[2])
        .sqrt()
        .max(1e-6);
    let sun_dir = Vec3::new(sd[0] / sd_len, sd[1] / sd_len, sd[2] / sd_len);
    let sun_color = Spectrum::rgb(
        sun.color[0] * sun.intensity,
        sun.color[1] * sun.intensity,
        sun.color[2] * sun.intensity,
    );
    let sky_color = Spectrum::rgb(
        sky.color[0] * sky.intensity,
        sky.color[1] * sky.intensity,
        sky.color[2] * sky.intensity,
    );

    let tiles_x = width.div_ceil(tile_size);
    let tiles_y = height.div_ceil(tile_size);
    let tiles_total = tiles_x * tiles_y;
    let mut image = RgbImage::new(width, height);

    let tile_indices: Vec<(u32, u32)> = (0..tiles_y)
        .flat_map(|ty| (0..tiles_x).map(move |tx| (tx, ty)))
        .collect();

    let (sender, receiver) = mpsc::channel::<(u32, u32, Vec<u8>)>();

    tile_indices
        .par_iter()
        .for_each_with(sender, |s, &(tile_x, tile_y)| {
            let sampler = sampler.clone();
            let px_start = tile_x * tile_size;
            let py_start = tile_y * tile_size;
            let px_end = (px_start + tile_size).min(width);
            let py_end = (py_start + tile_size).min(height);
            let tile_w = (px_end - px_start) as usize;
            let tile_h = (py_end - py_start) as usize;
            let mut pixels = vec![0u8; tile_w * tile_h * 3];

            for (iy, py) in (py_start..py_end).enumerate() {
                for (ix, px) in (px_start..px_end).enumerate() {
                    let ndc_x = ((px as f32 + 0.5) / width as f32 * 2.0 - 1.0) * aspect;
                    let ndc_y = 1.0 - (py as f32 + 0.5) / height as f32 * 2.0;

                    let ray = pinhole.generate_ray(ndc_x, ndc_y);

                    let color = trace_terrain(
                        &sampler,
                        compiled_eval.as_ref(),
                        gpu_field_raster.as_ref(),
                        material_lanes.as_ref(),
                        material_debug_mode,
                        ray.origin,
                        ray.direction,
                        sun_dir,
                        sun_color,
                        sky_color,
                        scene,
                        0, // Whitted depth
                    );

                    let rgb = spectrum_to_rgb8_reinhard(color);
                    let idx = (iy * tile_w + ix) * 3;
                    pixels[idx] = rgb[0];
                    pixels[idx + 1] = rgb[1];
                    pixels[idx + 2] = rgb[2];
                }
            }

            let _ = s.send((px_start, py_start, pixels));
        });

    let start = Instant::now();
    let mut tiles_done = 0u32;
    for (px_start, py_start, pixels) in receiver {
        let px_end = (px_start + tile_size).min(width);
        let py_end = (py_start + tile_size).min(height);
        let tile_w = (px_end - px_start) as usize;
        for (iy, py) in (py_start..py_end).enumerate() {
            for (ix, px) in (px_start..px_end).enumerate() {
                let idx = (iy * tile_w + ix) * 3;
                image.put_pixel(
                    px,
                    py,
                    image::Rgb([pixels[idx], pixels[idx + 1], pixels[idx + 2]]),
                );
            }
        }
        tiles_done += 1;
        if let Err(err) = progress_cb(
            RenderProgress {
                tiles_done,
                tiles_total,
                elapsed_ms: start.elapsed().as_millis(),
            },
            &image,
        ) {
            return Err(RenderError::Image(err));
        }
    }

    Ok(image)
}

fn render_material_preview_sphere<F>(
    width: u32,
    height: u32,
    tile_size: u32,
    sun: &GraphSun,
    sky: &GraphSky,
    _scene: &GraphSceneSettings,
    material_lanes: Option<&TerrainMaterialLanes<'_>>,
    material_debug_mode: MaterialDebugMode,
    mut progress_cb: F,
) -> Result<RgbImage, RenderError>
where
    F: FnMut(RenderProgress, &RgbImage) -> Result<(), ImageError> + Send,
{
    let width = width.max(1);
    let height = height.max(1);
    let tile_size = tile_size.max(8);
    let aspect = width as f32 / height as f32;

    let pinhole = PinholeCamera {
        origin: Vec3::new(0.5, 0.47, -0.44),
        target: Vec3::new(0.5, 0.45, 0.5),
        up: Vec3::new(0.0, 1.0, 0.0),
        fov_y_degrees: 33.0,
    };

    // Material preview uses a camera-side raking light so shell relief reads clearly.
    let cam_fwd = vec3_normalize(Vec3::new(
        pinhole.target.x - pinhole.origin.x,
        pinhole.target.y - pinhole.origin.y,
        pinhole.target.z - pinhole.origin.z,
    ));
    let cam_right = vec3_normalize(Vec3::new(cam_fwd.z, 0.0, -cam_fwd.x));
    let cam_up = vec3_normalize(Vec3::new(pinhole.up.x, pinhole.up.y, pinhole.up.z));
    let sun_dir = vec3_normalize(Vec3::new(
        -cam_fwd.x + cam_right.x * 0.70 + cam_up.x * 0.35,
        -cam_fwd.y + cam_right.y * 0.70 + cam_up.y * 0.35,
        -cam_fwd.z + cam_right.z * 0.70 + cam_up.z * 0.35,
    ));
    let sun_color = Spectrum::rgb(
        sun.color[0] * sun.intensity,
        sun.color[1] * sun.intensity,
        sun.color[2] * sun.intensity,
    );
    let sky_color = Spectrum::rgb(
        sky.color[0] * sky.intensity,
        sky.color[1] * sky.intensity,
        sky.color[2] * sky.intensity,
    );

    let sphere_center = Vec3::new(0.5, 0.45, 0.5);
    let sphere_radius = 0.26_f32;
    let proxy_extend = material_lanes
        .and_then(|lanes| lanes.bundle.as_ref())
        .map(|bundle| {
            let preview_p = Vec3::new(
                sphere_center.x,
                sphere_center.y + sphere_radius,
                sphere_center.z,
            );
            let preview_n = Vec3::new(0.0, 1.0, 0.0);
            let preview_v = vec3_normalize(Vec3::new(
                pinhole.origin.x - preview_p.x,
                pinhole.origin.y - preview_p.y,
                pinhole.origin.z - preview_p.z,
            ));
            bundle.max_extend(preview_p, preview_n, preview_v)
        })
        .unwrap_or(0.0);
    let camera_dist = ((pinhole.origin.x - sphere_center.x).powi(2)
        + (pinhole.origin.y - sphere_center.y).powi(2)
        + (pinhole.origin.z - sphere_center.z).powi(2))
    .sqrt();
    let proxy_radius = (sphere_radius + proxy_extend).min((camera_dist - 0.05).max(sphere_radius));

    let tiles_x = width.div_ceil(tile_size);
    let tiles_y = height.div_ceil(tile_size);
    let tiles_total = tiles_x * tiles_y;
    let mut image = RgbImage::new(width, height);

    let tile_indices: Vec<(u32, u32)> = (0..tiles_y)
        .flat_map(|ty| (0..tiles_x).map(move |tx| (tx, ty)))
        .collect();
    let (sender, receiver) = mpsc::channel::<(u32, u32, Vec<u8>)>();

    tile_indices
        .par_iter()
        .for_each_with(sender, |s, &(tile_x, tile_y)| {
            let px_start = tile_x * tile_size;
            let py_start = tile_y * tile_size;
            let px_end = (px_start + tile_size).min(width);
            let py_end = (py_start + tile_size).min(height);
            let tile_w = (px_end - px_start) as usize;
            let tile_h = (py_end - py_start) as usize;
            let mut pixels = vec![0u8; tile_w * tile_h * 3];

            for (iy, py) in (py_start..py_end).enumerate() {
                for (ix, px) in (px_start..px_end).enumerate() {
                    let ndc_x = ((px as f32 + 0.5) / width as f32 * 2.0 - 1.0) * aspect;
                    let ndc_y = 1.0 - (py as f32 + 0.5) / height as f32 * 2.0;
                    let ray = pinhole.generate_ray(ndc_x, ndc_y);

                    let color = if let Some(t) =
                        intersect_sphere(ray.origin, ray.direction, sphere_center, proxy_radius)
                    {
                        let p = vec3_fma(ray.origin, ray.direction, t);
                        let n = vec3_normalize(Vec3::new(
                            p.x - sphere_center.x,
                            p.y - sphere_center.y,
                            p.z - sphere_center.z,
                        ));
                        let shell_hit = march_material_shell_sphere(
                            material_lanes,
                            _scene,
                            sphere_center,
                            sphere_radius,
                            p,
                            n,
                            ray.direction,
                        );
                        if let Some((sp, sn)) = shell_hit {
                            shade_preview_material(
                                material_lanes,
                                material_debug_mode,
                                sp,
                                sn,
                                ray.direction,
                                sun_dir,
                                sun_color,
                                sky_color,
                            )
                        } else {
                            sky_radiance(ray.direction, sun_dir, sky_color)
                        }
                    } else {
                        sky_radiance(ray.direction, sun_dir, sky_color)
                    };

                    let rgb = spectrum_to_rgb8_reinhard(color);
                    let idx = (iy * tile_w + ix) * 3;
                    pixels[idx] = rgb[0];
                    pixels[idx + 1] = rgb[1];
                    pixels[idx + 2] = rgb[2];
                }
            }

            let _ = s.send((px_start, py_start, pixels));
        });

    let start = Instant::now();
    let mut tiles_done = 0u32;
    for (px_start, py_start, pixels) in receiver {
        let px_end = (px_start + tile_size).min(width);
        let py_end = (py_start + tile_size).min(height);
        let tile_w = (px_end - px_start) as usize;
        for (iy, py) in (py_start..py_end).enumerate() {
            for (ix, px) in (px_start..px_end).enumerate() {
                let idx = (iy * tile_w + ix) * 3;
                image.put_pixel(
                    px,
                    py,
                    image::Rgb([pixels[idx], pixels[idx + 1], pixels[idx + 2]]),
                );
            }
        }
        tiles_done += 1;
        if let Err(err) = progress_cb(
            RenderProgress {
                tiles_done,
                tiles_total,
                elapsed_ms: start.elapsed().as_millis(),
            },
            &image,
        ) {
            return Err(RenderError::Image(err));
        }
    }

    Ok(image)
}

fn shade_preview_material(
    material_lanes: Option<&TerrainMaterialLanes<'_>>,
    material_debug_mode: MaterialDebugMode,
    p: Vec3,
    n: Vec3,
    view_dir: Vec3,
    sun_dir: Vec3,
    sun_color: Spectrum,
    sky_color: Spectrum,
) -> Spectrum {
    let material = sample_surface_material(material_lanes, p, n, view_dir, material_debug_mode);
    let albedo = material.base_color;
    let n_dot_l = n.dot(sun_dir).max(0.0);
    let direct = spectrum_mul(albedo, sun_color.scale(n_dot_l * 1.2));
    let ambient = spectrum_mul(albedo, sky_color.scale(0.12 + 0.22 * n.y.clamp(0.0, 1.0)));

    let wo = vec3_neg(view_dir);
    let n_dot_v = n.dot(wo).max(0.0);
    let base_luma = luminance(albedo).clamp(0.0, 1.0);
    let f0 = lerp_f32(0.04, base_luma, material.metallic.clamp(0.0, 1.0));
    let fresnel = f0 + (1.0 - f0) * (1.0 - n_dot_v).powf(5.0);
    let reflect_dir = reflect(view_dir, n);
    let env = sky_radiance(reflect_dir, sun_dir, sky_color);
    let spec = env.scale(
        fresnel
            * (1.0 - material.roughness.clamp(0.0, 1.0)).powf(1.2)
            * (1.0 - material.transparency.clamp(0.0, 1.0)),
    );
    direct + ambient + spec
}

fn intersect_sphere(ro: Vec3, rd: Vec3, c: Vec3, r: f32) -> Option<f32> {
    let oc = Vec3::new(ro.x - c.x, ro.y - c.y, ro.z - c.z);
    let b = oc.x * rd.x + oc.y * rd.y + oc.z * rd.z;
    let cterm = oc.x * oc.x + oc.y * oc.y + oc.z * oc.z - r * r;
    let h = b * b - cterm;
    if h < 0.0 {
        return None;
    }
    let s = h.sqrt();
    let t0 = -b - s;
    let t1 = -b + s;
    if t0 > 1.0e-4 {
        Some(t0)
    } else if t1 > 1.0e-4 {
        Some(t1)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Height sampling
// ---------------------------------------------------------------------------

fn sample_height(
    sampler: &HeightSampler<'_>,
    compiled: Option<&crate::jit::JitFunction>,
    gpu_field: Option<&GpuFieldRaster>,
    scene: &GraphSceneSettings,
    x: f32,
    z: f32,
) -> f32 {
    let h = if let Some(raster) = gpu_field {
        raster.sample_bilinear(x, z)
    } else {
        compiled
            .and_then(|jit| jit.invoke(&[x, z]))
            .or_else(|| sampler.sample_root_scalar(x, z))
            .unwrap_or(0.0)
            .clamp(0.0, 1.0)
    };
    h * scene.height_scale
}

// ---------------------------------------------------------------------------
// Raymarching
// ---------------------------------------------------------------------------

/// March a ray against the heightfield. Returns `(hit_point, normal)` or `None`.
fn march_terrain(
    sampler: &HeightSampler<'_>,
    compiled: Option<&crate::jit::JitFunction>,
    gpu_field: Option<&GpuFieldRaster>,
    ro: Vec3,
    rd: Vec3,
    scene: &GraphSceneSettings,
) -> Option<(Vec3, Vec3)> {
    let max_dist = scene.max_dist;
    let eps = scene.epsilon;
    let ws = scene.world_size;
    let (mut t, max_t) = ray_bounds_interval_xz(ro, rd, ws)?;

    if t > max_dist {
        return None;
    }
    let max_t = max_t.min(max_dist);

    for _ in 0..scene.max_steps {
        let p = vec3_fma(ro, rd, t);

        if t > max_t {
            return None;
        }

        let h = sample_height(sampler, compiled, gpu_field, scene, p.x, p.z);

        let diff = p.y - h;

        if diff < eps {
            // Refine to surface.
            let t_refined = (t - (eps - diff) * 0.5).max(0.0);
            let p_hit = vec3_fma(ro, rd, t_refined);
            let n = terrain_normal(sampler, compiled, gpu_field, scene, p_hit);
            return Some((p_hit, n));
        }

        // Sphere step: move by `diff` (height above surface), clamped.
        t += (diff * 0.5).max(0.001).min(0.5);
    }

    None
}

fn ray_bounds_interval_xz(ro: Vec3, rd: Vec3, world_size: f32) -> Option<(f32, f32)> {
    fn axis_interval(origin: f32, dir: f32, max: f32) -> Option<(f32, f32)> {
        if dir.abs() < 1.0e-6 {
            if (0.0..=max).contains(&origin) {
                Some((f32::NEG_INFINITY, f32::INFINITY))
            } else {
                None
            }
        } else {
            let t0 = (0.0 - origin) / dir;
            let t1 = (max - origin) / dir;
            Some((t0.min(t1), t0.max(t1)))
        }
    }

    let (x0, x1) = axis_interval(ro.x, rd.x, world_size)?;
    let (z0, z1) = axis_interval(ro.z, rd.z, world_size)?;
    let enter = x0.max(z0).max(0.0);
    let exit = x1.min(z1);
    (enter <= exit).then_some((enter, exit))
}

/// Test whether the sun is visible from a surface point (shadow ray).
/// Returns `true` if unoccluded (lit), `false` if in shadow.
fn sun_visible(
    sampler: &HeightSampler<'_>,
    compiled: Option<&crate::jit::JitFunction>,
    gpu_field: Option<&GpuFieldRaster>,
    scene: &GraphSceneSettings,
    p: Vec3,
    sun_dir: Vec3,
    n: Vec3,
) -> bool {
    // Offset origin along normal to avoid self-intersection.
    let offset = scene.normal_eps * 8.0;
    let origin = Vec3::new(p.x + n.x * offset, p.y + n.y * offset, p.z + n.z * offset);
    march_terrain(sampler, compiled, gpu_field, origin, sun_dir, scene).is_none()
}

// ---------------------------------------------------------------------------
// Whitted integrator
// ---------------------------------------------------------------------------

/// Recursive Whitted shading. `depth` caps at 1 for one specular reflection bounce.
fn trace_terrain(
    sampler: &HeightSampler<'_>,
    compiled: Option<&crate::jit::JitFunction>,
    gpu_field: Option<&GpuFieldRaster>,
    material_lanes: Option<&TerrainMaterialLanes<'_>>,
    material_debug_mode: MaterialDebugMode,
    ro: Vec3,
    rd: Vec3,
    sun_dir: Vec3,
    sun_color: Spectrum,
    sky_color: Spectrum,
    scene: &GraphSceneSettings,
    depth: u32,
) -> Spectrum {
    match march_terrain(sampler, compiled, gpu_field, ro, rd, scene) {
        None => sky_radiance(rd, sun_dir, sky_color),
        Some((p, n)) => shade_hit(
            sampler,
            compiled,
            gpu_field,
            material_lanes,
            material_debug_mode,
            scene,
            p,
            n,
            rd,
            sun_dir,
            sun_color,
            sky_color,
            depth,
        ),
    }
}

fn shade_hit(
    sampler: &HeightSampler<'_>,
    compiled: Option<&crate::jit::JitFunction>,
    gpu_field: Option<&GpuFieldRaster>,
    material_lanes: Option<&TerrainMaterialLanes<'_>>,
    material_debug_mode: MaterialDebugMode,
    scene: &GraphSceneSettings,
    p: Vec3,
    n: Vec3,
    view_dir: Vec3,
    sun_dir: Vec3,
    sun_color: Spectrum,
    sky_color: Spectrum,
    depth: u32,
) -> Spectrum {
    let (shade_p, shade_n) = if depth == 0 {
        march_material_shell(material_lanes, scene, p, n, view_dir).unwrap_or((p, n))
    } else {
        (p, n)
    };
    let material = sample_surface_material(
        material_lanes,
        shade_p,
        shade_n,
        view_dir,
        material_debug_mode,
    );
    let albedo = material.base_color;

    // --- Shadow ray (hard shadows) ---
    let lit = sun_visible(
        sampler, compiled, gpu_field, scene, shade_p, sun_dir, shade_n,
    );

    // --- Direct illumination: Lambert diffuse ---
    let n_dot_l = shade_n.dot(sun_dir).max(0.0);
    let diffuse_weight = (1.0 - material.transparency).clamp(0.0, 1.0);
    let direct = if lit {
        spectrum_mul(albedo, sun_color.scale(n_dot_l * diffuse_weight))
    } else {
        // In shadow: only low-frequency bounce light (no direct sun).
        spectrum_mul(albedo, sun_color.scale(n_dot_l * 0.05 * diffuse_weight))
    };

    // --- Sky ambient (hemisphere approximation) ---
    // Flat surfaces (n.y ≈ 1) see the full sky dome; vertical faces see half.
    let sky_weight = 0.5 + 0.5 * shade_n.y.clamp(0.0, 1.0);
    let ambient = spectrum_mul(albedo, sky_color.scale(sky_weight * diffuse_weight));

    // --- Specular: Whitted reflection ray with Fresnel-Schlick ---
    let specular = if depth < 1 {
        let wo = vec3_neg(view_dir);
        let n_dot_v = shade_n.dot(wo).max(0.0);
        let base_luma = luminance(albedo).clamp(0.0, 1.0);
        let f0 = lerp_f32(0.04, base_luma, material.metallic.clamp(0.0, 1.0));
        let fresnel = f0 + (1.0 - f0) * (1.0 - n_dot_v).powf(5.0);
        let coat_reflect =
            material.coat.clamp(0.0, 1.0) * (1.0 - material.coat_roughness.clamp(0.0, 1.0));
        let microsurface = (1.0 - material.roughness.clamp(0.0, 1.0)).powf(1.6);
        let reflect_weight = (microsurface + coat_reflect).clamp(0.0, 1.0);

        if fresnel > 1e-4 && reflect_weight > 1e-4 {
            let reflect_dir = reflect(view_dir, shade_n);
            let offset = scene.normal_eps * 8.0;
            let reflect_origin = Vec3::new(
                shade_p.x + shade_n.x * offset,
                shade_p.y + shade_n.y * offset,
                shade_p.z + shade_n.z * offset,
            );
            let reflected = trace_terrain(
                sampler,
                compiled,
                gpu_field,
                material_lanes,
                material_debug_mode,
                reflect_origin,
                reflect_dir,
                sun_dir,
                sun_color,
                sky_color,
                scene,
                depth + 1,
            );
            reflected.scale(fresnel * reflect_weight)
        } else {
            Spectrum::black()
        }
    } else {
        Spectrum::black()
    };

    let transmission = if material.transparency > 0.0 {
        sky_radiance(view_dir, sun_dir, sky_color).scale(material.transparency * 0.18)
    } else {
        Spectrum::black()
    };

    direct + ambient + specular + transmission
}

#[derive(Clone, Copy)]
struct TerrainShadingMaterial {
    base_color: Spectrum,
    roughness: f32,
    metallic: f32,
    coat: f32,
    coat_roughness: f32,
    transparency: f32,
}

enum MaterialNodeBundle<'a> {
    Leaf {
        base_color: MaterialColorSource<'a>,
        displacement: MaterialScalarSource<'a>,
        max_extend: MaterialScalarSource<'a>,
        roughness: MaterialScalarSource<'a>,
        metallic: MaterialScalarSource<'a>,
        coat: MaterialScalarSource<'a>,
        coat_roughness: MaterialScalarSource<'a>,
        transparency: MaterialScalarSource<'a>,
    },
    Blend {
        a: Box<MaterialNodeBundle<'a>>,
        b: Box<MaterialNodeBundle<'a>>,
        mask: MaterialScalarSource<'a>,
    },
}

impl<'a> MaterialNodeBundle<'a> {
    fn from_state(state: &'a EvalState) -> Option<Self> {
        let binding = state.bindings.get("graph_material")?;
        let target = match &binding.value {
            Value::Object(obj) => obj.clone(),
            _ => return None,
        };
        Self::from_object(state, &target)
    }

    fn from_object(state: &'a EvalState, target: &ObjectValue) -> Option<Self> {
        match target.type_name.as_deref()? {
            "Material" => Some(Self::Leaf {
                base_color: MaterialColorSource::from_value(state, target.fields.get("base_color")),
                displacement: MaterialScalarSource::from_value(
                    state,
                    target.fields.get("displacement"),
                ),
                max_extend: MaterialScalarSource::from_value_or(
                    state,
                    target.fields.get("max_extend"),
                    0.25,
                ),
                roughness: MaterialScalarSource::from_value_or(
                    state,
                    target.fields.get("roughness"),
                    0.7,
                ),
                metallic: MaterialScalarSource::from_value_or(
                    state,
                    target.fields.get("metallic"),
                    0.0,
                ),
                coat: MaterialScalarSource::from_value_or(state, target.fields.get("coat"), 0.0),
                coat_roughness: MaterialScalarSource::from_value_or(
                    state,
                    target.fields.get("coat_roughness"),
                    0.0,
                ),
                transparency: MaterialScalarSource::from_value_or(
                    state,
                    target.fields.get("transparency"),
                    0.0,
                ),
            }),
            "MaterialBlend" => {
                let a = match target.fields.get("a")? {
                    Value::Object(obj) => Self::from_object(state, obj)?,
                    _ => return None,
                };
                let b = match target.fields.get("b")? {
                    Value::Object(obj) => Self::from_object(state, obj)?,
                    _ => return None,
                };
                Some(Self::Blend {
                    a: Box::new(a),
                    b: Box::new(b),
                    mask: MaterialScalarSource::from_value_or(
                        state,
                        target.fields.get("mask"),
                        0.5,
                    ),
                })
            }
            _ => None,
        }
    }

    fn base_color(&self, p: Vec3, n: Vec3, view_dir: Vec3) -> Spectrum {
        match self {
            Self::Leaf {
                base_color,
                displacement,
                ..
            } => {
                let density = displacement.sample_abs_unit(p, n, view_dir).unwrap_or(0.0);
                base_color
                    .sample(p, n, view_dir)
                    .unwrap_or(Spectrum::rgb(density, density, density))
            }
            Self::Blend { a, b, mask } => {
                let t = mask.sample_unit(p, n, view_dir).unwrap_or(0.5);
                lerp_spectrum(
                    a.base_color(p, n, view_dir),
                    b.base_color(p, n, view_dir),
                    t,
                )
            }
        }
    }

    fn displacement(&self, p: Vec3, n: Vec3, view_dir: Vec3) -> f32 {
        match self {
            Self::Leaf { displacement, .. } => {
                displacement.sample_raw(p, n, view_dir).unwrap_or(0.0)
            }
            Self::Blend { a, b, mask } => {
                let t = mask.sample_unit(p, n, view_dir).unwrap_or(0.5);
                lerp_f32(
                    a.displacement(p, n, view_dir),
                    b.displacement(p, n, view_dir),
                    t,
                )
            }
        }
    }

    fn roughness(&self, p: Vec3, n: Vec3, view_dir: Vec3) -> f32 {
        match self {
            Self::Leaf { roughness, .. } => roughness.sample_unit(p, n, view_dir).unwrap_or(0.7),
            Self::Blend { a, b, mask } => {
                let t = mask.sample_unit(p, n, view_dir).unwrap_or(0.5);
                lerp_f32(a.roughness(p, n, view_dir), b.roughness(p, n, view_dir), t)
            }
        }
    }

    fn max_extend(&self, p: Vec3, n: Vec3, view_dir: Vec3) -> f32 {
        match self {
            Self::Leaf { max_extend, .. } => max_extend
                .sample_raw(p, n, view_dir)
                .unwrap_or(0.25)
                .max(0.0),
            Self::Blend { a, b, .. } => a
                .max_extend(p, n, view_dir)
                .max(b.max_extend(p, n, view_dir)),
        }
    }

    fn metallic(&self, p: Vec3, n: Vec3, view_dir: Vec3) -> f32 {
        match self {
            Self::Leaf { metallic, .. } => metallic.sample_unit(p, n, view_dir).unwrap_or(0.0),
            Self::Blend { a, b, mask } => {
                let t = mask.sample_unit(p, n, view_dir).unwrap_or(0.5);
                lerp_f32(a.metallic(p, n, view_dir), b.metallic(p, n, view_dir), t)
            }
        }
    }

    fn coat(&self, p: Vec3, n: Vec3, view_dir: Vec3) -> f32 {
        match self {
            Self::Leaf { coat, .. } => coat.sample_unit(p, n, view_dir).unwrap_or(0.0),
            Self::Blend { a, b, mask } => {
                let t = mask.sample_unit(p, n, view_dir).unwrap_or(0.5);
                lerp_f32(a.coat(p, n, view_dir), b.coat(p, n, view_dir), t)
            }
        }
    }

    fn coat_roughness(&self, p: Vec3, n: Vec3, view_dir: Vec3) -> f32 {
        match self {
            Self::Leaf { coat_roughness, .. } => {
                coat_roughness.sample_unit(p, n, view_dir).unwrap_or(0.0)
            }
            Self::Blend { a, b, mask } => {
                let t = mask.sample_unit(p, n, view_dir).unwrap_or(0.5);
                lerp_f32(
                    a.coat_roughness(p, n, view_dir),
                    b.coat_roughness(p, n, view_dir),
                    t,
                )
            }
        }
    }

    fn transparency(&self, p: Vec3, n: Vec3, view_dir: Vec3) -> f32 {
        match self {
            Self::Leaf { transparency, .. } => {
                transparency.sample_unit(p, n, view_dir).unwrap_or(0.0)
            }
            Self::Blend { a, b, mask } => {
                let t = mask.sample_unit(p, n, view_dir).unwrap_or(0.5);
                lerp_f32(
                    a.transparency(p, n, view_dir),
                    b.transparency(p, n, view_dir),
                    t,
                )
            }
        }
    }
}

fn sample_surface_material(
    material_lanes: Option<&TerrainMaterialLanes<'_>>,
    p: Vec3,
    n: Vec3,
    view_dir: Vec3,
    debug_mode: MaterialDebugMode,
) -> TerrainShadingMaterial {
    let base = TerrainShadingMaterial {
        base_color: Spectrum::rgb(0.70, 0.68, 0.65),
        roughness: 0.68,
        metallic: 0.03,
        coat: 0.0,
        coat_roughness: 0.0,
        transparency: 0.0,
    };

    let Some(lanes) = material_lanes else {
        return base;
    };
    let Some(bundle) = lanes.bundle.as_ref() else {
        return base;
    };
    let density = bundle.displacement(p, n, view_dir).abs().clamp(0.0, 1.0);
    let mut color = bundle.base_color(p, n, view_dir);
    if matches!(debug_mode, MaterialDebugMode::Mask) {
        color = Spectrum::rgb(density, 1.0 - density, 0.0);
    } else if matches!(debug_mode, MaterialDebugMode::Lanes) {
        color = Spectrum::rgb(
            bundle.roughness(p, n, view_dir),
            bundle.metallic(p, n, view_dir),
            bundle.coat(p, n, view_dir),
        );
    }
    TerrainShadingMaterial {
        base_color: color,
        roughness: bundle.roughness(p, n, view_dir),
        metallic: bundle.metallic(p, n, view_dir),
        coat: bundle.coat(p, n, view_dir),
        coat_roughness: bundle.coat_roughness(p, n, view_dir),
        transparency: bundle.transparency(p, n, view_dir),
    }
}

fn march_material_shell(
    material_lanes: Option<&TerrainMaterialLanes<'_>>,
    scene: &GraphSceneSettings,
    macro_p: Vec3,
    macro_n: Vec3,
    view_dir: Vec3,
) -> Option<(Vec3, Vec3)> {
    let lanes = material_lanes?;
    let eps = scene.epsilon.max(0.001);
    let max_t = shell_search_radius(scene, lanes, macro_p, macro_n, view_dir);
    let mut t = 0.0_f32;
    let start = macro_p;
    let march_dir = view_dir;
    for _ in 0..64 {
        if t > max_t {
            break;
        }
        let p = vec3_fma(start, march_dir, t);
        let f = shell_implicit(lanes, p, macro_p, macro_n, view_dir)?;
        if f.abs() <= eps {
            let n = shell_normal_fast(lanes, p, macro_p, macro_n, view_dir, eps)?;
            return Some((p, n));
        }
        t += f;
    }

    None
}

fn march_material_shell_sphere(
    material_lanes: Option<&TerrainMaterialLanes<'_>>,
    scene: &GraphSceneSettings,
    sphere_center: Vec3,
    sphere_radius: f32,
    macro_p: Vec3,
    _macro_n: Vec3,
    view_dir: Vec3,
) -> Option<(Vec3, Vec3)> {
    let lanes = material_lanes?;
    let eps = scene.epsilon.max(0.001);
    let max_t = shell_search_radius(scene, lanes, macro_p, Vec3::new(0.0, 1.0, 0.0), view_dir);
    let mut t = 0.0_f32;
    let start = macro_p;
    // let f0 = shell_implicit_sphere(lanes, start, sphere_center, sphere_radius, view_dir)?;
    // if f0.abs() <= eps {
    //     let n =
    //         shell_normal_fast_sphere(lanes, start, sphere_center, sphere_radius, view_dir, eps)?;
    //     return Some((start, n));
    // }
    // let march_dir = if f0 < 0.0 {
    //     vec3_neg(view_dir)
    // } else {
    //     view_dir
    // };
    let march_dir = view_dir;
    for _ in 0..64 {
        if t > max_t {
            break;
        }
        let p = vec3_fma(start, march_dir, t);
        let f = shell_implicit_sphere(lanes, p, sphere_center, sphere_radius, view_dir)?;
        if f.abs() <= eps {
            let n =
                shell_normal_fast_sphere(lanes, p, sphere_center, sphere_radius, view_dir, eps)?;
            return Some((p, n));
        }
        t += f; //.abs().max(1.5e-4);
    }

    None
}

fn shell_implicit(
    lanes: &TerrainMaterialLanes<'_>,
    p: Vec3,
    macro_p: Vec3,
    macro_n: Vec3,
    view_dir: Vec3,
) -> Option<f32> {
    let disp = if let Some(bundle) = lanes.bundle.as_ref() {
        bundle.displacement(p, macro_n, view_dir)
    } else {
        return None;
    };
    let base = (p.x - macro_p.x) * macro_n.x
        + (p.y - macro_p.y) * macro_n.y
        + (p.z - macro_p.z) * macro_n.z;
    Some(base + disp)
}

fn shell_implicit_sphere(
    lanes: &TerrainMaterialLanes<'_>,
    p: Vec3,
    sphere_center: Vec3,
    sphere_radius: f32,
    view_dir: Vec3,
) -> Option<f32> {
    let sphere_normal = vec3_normalize(Vec3::new(
        p.x - sphere_center.x,
        p.y - sphere_center.y,
        p.z - sphere_center.z,
    ));
    let disp = if let Some(bundle) = lanes.bundle.as_ref() {
        bundle.displacement(p, sphere_normal, view_dir)
    } else {
        return None;
    };
    let base = ((p.x - sphere_center.x).powi(2)
        + (p.y - sphere_center.y).powi(2)
        + (p.z - sphere_center.z).powi(2))
    .sqrt()
        - sphere_radius;
    Some(base + disp)
}

fn shell_normal_fast(
    lanes: &TerrainMaterialLanes<'_>,
    p: Vec3,
    macro_p: Vec3,
    macro_n: Vec3,
    view_dir: Vec3,
    eps: f32,
) -> Option<Vec3> {
    // Tetrahedral gradient: 4 samples instead of 6 central differences.
    let k = 0.57735026 * eps;
    let e0 = Vec3::new(k, -k, -k);
    let e1 = Vec3::new(-k, -k, k);
    let e2 = Vec3::new(-k, k, -k);
    let e3 = Vec3::new(k, k, k);
    let f0 = shell_implicit(lanes, vec3_add(p, e0), macro_p, macro_n, view_dir)?;
    let f1 = shell_implicit(lanes, vec3_add(p, e1), macro_p, macro_n, view_dir)?;
    let f2 = shell_implicit(lanes, vec3_add(p, e2), macro_p, macro_n, view_dir)?;
    let f3 = shell_implicit(lanes, vec3_add(p, e3), macro_p, macro_n, view_dir)?;
    let gx = e0.x * f0 + e1.x * f1 + e2.x * f2 + e3.x * f3;
    let gy = e0.y * f0 + e1.y * f1 + e2.y * f2 + e3.y * f3;
    let gz = e0.z * f0 + e1.z * f1 + e2.z * f2 + e3.z * f3;
    Some(vec3_normalize(Vec3::new(gx, gy, gz)))
}

fn shell_normal_fast_sphere(
    lanes: &TerrainMaterialLanes<'_>,
    p: Vec3,
    sphere_center: Vec3,
    sphere_radius: f32,
    view_dir: Vec3,
    eps: f32,
) -> Option<Vec3> {
    let k = 0.57735026 * eps;
    let e0 = Vec3::new(k, -k, -k);
    let e1 = Vec3::new(-k, -k, k);
    let e2 = Vec3::new(-k, k, -k);
    let e3 = Vec3::new(k, k, k);
    let f0 = shell_implicit_sphere(
        lanes,
        vec3_add(p, e0),
        sphere_center,
        sphere_radius,
        view_dir,
    )?;
    let f1 = shell_implicit_sphere(
        lanes,
        vec3_add(p, e1),
        sphere_center,
        sphere_radius,
        view_dir,
    )?;
    let f2 = shell_implicit_sphere(
        lanes,
        vec3_add(p, e2),
        sphere_center,
        sphere_radius,
        view_dir,
    )?;
    let f3 = shell_implicit_sphere(
        lanes,
        vec3_add(p, e3),
        sphere_center,
        sphere_radius,
        view_dir,
    )?;
    let gx = e0.x * f0 + e1.x * f1 + e2.x * f2 + e3.x * f3;
    let gy = e0.y * f0 + e1.y * f1 + e2.y * f2 + e3.y * f3;
    let gz = e0.z * f0 + e1.z * f1 + e2.z * f2 + e3.z * f3;
    Some(vec3_normalize(Vec3::new(gx, gy, gz)))
}

fn shell_search_radius(
    scene: &GraphSceneSettings,
    lanes: &TerrainMaterialLanes<'_>,
    p: Vec3,
    n: Vec3,
    view_dir: Vec3,
) -> f32 {
    let extend = lanes
        .bundle
        .as_ref()
        .map(|bundle| bundle.max_extend(p, n, view_dir))
        .unwrap_or(0.25);
    extend
        .max(scene.normal_eps * 128.0)
        .max(scene.epsilon * 4.0)
        .min(scene.max_dist.max(0.25))
}

fn shell_context(pos: Vec3, normal: Vec3, view_dir: Vec3) -> ObjectValue {
    ObjectValue {
        type_name: Some("NodeContext".to_string()),
        fields: std::collections::HashMap::from([
            ("stage".to_string(), Value::String("shell".to_string())),
            (
                "pos2d".to_string(),
                Value::Object(ObjectValue {
                    type_name: Some("vec3".to_string()),
                    fields: std::collections::HashMap::from([
                        ("x".to_string(), Value::Number(pos.x)),
                        ("y".to_string(), Value::Number(0.0)),
                        ("z".to_string(), Value::Number(pos.z)),
                    ]),
                }),
            ),
            (
                "pos3d".to_string(),
                Value::Object(ObjectValue {
                    type_name: Some("vec3".to_string()),
                    fields: std::collections::HashMap::from([
                        ("x".to_string(), Value::Number(pos.x)),
                        ("y".to_string(), Value::Number(pos.y)),
                        ("z".to_string(), Value::Number(pos.z)),
                    ]),
                }),
            ),
            (
                "normal".to_string(),
                Value::Object(ObjectValue {
                    type_name: Some("vec3".to_string()),
                    fields: std::collections::HashMap::from([
                        ("x".to_string(), Value::Number(normal.x)),
                        ("y".to_string(), Value::Number(normal.y)),
                        ("z".to_string(), Value::Number(normal.z)),
                    ]),
                }),
            ),
            (
                "view_dir".to_string(),
                Value::Object(ObjectValue {
                    type_name: Some("vec3".to_string()),
                    fields: std::collections::HashMap::from([
                        ("x".to_string(), Value::Number(view_dir.x)),
                        ("y".to_string(), Value::Number(view_dir.y)),
                        ("z".to_string(), Value::Number(view_dir.z)),
                    ]),
                }),
            ),
            ("height".to_string(), Value::Number(0.0)),
            ("slope".to_string(), Value::Number(0.0)),
            ("mask".to_string(), Value::Number(0.0)),
            ("value".to_string(), Value::Number(0.0)),
        ]),
    }
}

struct MaterialScalarLane<'a> {
    state: &'a EvalState,
    target: ObjectValue,
    node_name: String,
    compiled: Option<crate::jit::JitFunction>,
}

impl<'a> MaterialScalarLane<'a> {
    fn sample_shell_raw(&self, p: Vec3, n: Vec3, view_dir: Vec3) -> Option<f32> {
        self.compiled
            .and_then(|compiled| {
                compiled.invoke(&[
                    p.x, p.y, p.z, n.x, n.y, n.z, view_dir.x, view_dir.y, view_dir.z,
                ])
            })
            .or_else(|| {
                let ctx = shell_context(p, n, view_dir);
                eval_node_function(
                    self.state,
                    &self.node_name,
                    Some(&self.target),
                    "eval",
                    &[Value::Object(ctx)],
                )
                .ok()
                .and_then(|v| match v {
                    Value::Number(n) => Some(n),
                    _ => None,
                })
            })
    }
}

enum MaterialScalarSource<'a> {
    Constant(f32),
    Node(MaterialScalarLane<'a>),
}

impl<'a> MaterialScalarSource<'a> {
    fn from_value(state: &'a EvalState, value: Option<&Value>) -> Self {
        match value {
            Some(Value::Number(v)) => Self::Constant(*v),
            Some(Value::Object(obj)) => Self::Node(MaterialScalarLane {
                state,
                target: obj.clone(),
                node_name: obj.type_name.clone().unwrap_or_default(),
                compiled: obj.type_name.as_deref().and_then(|node_name| {
                    compile_specialized_shell_node_eval_function(state, node_name, Some(obj))
                }),
            }),
            _ => Self::Constant(0.0),
        }
    }

    fn from_value_or(state: &'a EvalState, value: Option<&Value>, fallback: f32) -> Self {
        match value {
            Some(_) => Self::from_value(state, value),
            None => Self::Constant(fallback),
        }
    }

    fn sample_raw(&self, p: Vec3, n: Vec3, view_dir: Vec3) -> Option<f32> {
        match self {
            Self::Constant(v) => Some(*v),
            Self::Node(node) => node.sample_shell_raw(p, n, view_dir),
        }
    }

    fn sample_unit(&self, p: Vec3, n: Vec3, view_dir: Vec3) -> Option<f32> {
        self.sample_raw(p, n, view_dir).map(|v| v.clamp(0.0, 1.0))
    }

    fn sample_abs_unit(&self, p: Vec3, n: Vec3, view_dir: Vec3) -> Option<f32> {
        self.sample_raw(p, n, view_dir)
            .map(|v| v.abs().clamp(0.0, 1.0))
    }
}

enum MaterialColorSource<'a> {
    Constant(Spectrum),
    Scalar(MaterialScalarSource<'a>),
}

impl<'a> MaterialColorSource<'a> {
    fn from_value(state: &'a EvalState, value: Option<&Value>) -> Self {
        match value {
            Some(v) => {
                if let Some(color) = parse_color_value(v) {
                    Self::Constant(color)
                } else {
                    Self::Scalar(MaterialScalarSource::from_value(state, Some(v)))
                }
            }
            None => Self::Constant(Spectrum::rgb(0.7, 0.68, 0.65)),
        }
    }

    fn sample(&self, p: Vec3, n: Vec3, view_dir: Vec3) -> Option<Spectrum> {
        match self {
            Self::Constant(color) => Some(*color),
            Self::Scalar(source) => {
                let v = source.sample_unit(p, n, view_dir)?;
                Some(Spectrum::rgb(v, v, v))
            }
        }
    }
}

struct TerrainMaterialLanes<'a> {
    bundle: Option<MaterialNodeBundle<'a>>,
}

impl<'a> TerrainMaterialLanes<'a> {
    fn from_state(state: &'a EvalState) -> Option<Self> {
        MaterialNodeBundle::from_state(state).map(|bundle| Self {
            bundle: Some(bundle),
        })
    }
}

// ---------------------------------------------------------------------------
// Normal estimation
// ---------------------------------------------------------------------------

fn terrain_normal(
    sampler: &HeightSampler<'_>,
    compiled: Option<&crate::jit::JitFunction>,
    gpu_field: Option<&GpuFieldRaster>,
    scene: &GraphSceneSettings,
    p: Vec3,
) -> Vec3 {
    let e = scene.normal_eps;
    let h0 = sample_height(sampler, compiled, gpu_field, scene, p.x, p.z);
    let hx = sample_height(sampler, compiled, gpu_field, scene, p.x + e, p.z);
    let hz = sample_height(sampler, compiled, gpu_field, scene, p.x, p.z + e);
    vec3_normalize(Vec3::new(h0 - hx, e, h0 - hz))
}

// ---------------------------------------------------------------------------
// Sky
// ---------------------------------------------------------------------------

fn sky_radiance(rd: Vec3, sun_dir: Vec3, sky_color: Spectrum) -> Spectrum {
    let t = (rd.y * 0.5 + 0.5).clamp(0.0, 1.0);
    let horizon = Spectrum::rgb(0.72, 0.84, 0.96);
    let zenith = Spectrum::rgb(0.18, 0.42, 0.78);
    let sky = lerp_spectrum(horizon, zenith, t.powf(0.6));

    // Sun disc + glow halo.
    let sun_dot = rd.dot(sun_dir).max(0.0);
    let sun_disc = smoothstep(0.9985, 1.0, sun_dot);
    let sun_glow = smoothstep(0.96, 0.999, sun_dot) * 0.18;
    let sun_contrib = Spectrum::rgb(1.0, 0.95, 0.85).scale(sun_disc * 8.0 + sun_glow);

    // Tint sky by the configured sky color.
    sky + sky_color.scale(0.3 * t) + sun_contrib
}

// ---------------------------------------------------------------------------
// Math helpers
// ---------------------------------------------------------------------------

/// `ro + rd * t`
#[inline]
fn vec3_fma(ro: Vec3, rd: Vec3, t: f32) -> Vec3 {
    Vec3::new(ro.x + rd.x * t, ro.y + rd.y * t, ro.z + rd.z * t)
}

#[inline]
fn vec3_neg(v: Vec3) -> Vec3 {
    Vec3::new(-v.x, -v.y, -v.z)
}

#[inline]
fn vec3_normalize(v: Vec3) -> Vec3 {
    let len = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt().max(1e-6);
    Vec3::new(v.x / len, v.y / len, v.z / len)
}

#[inline]
fn vec3_add(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(a.x + b.x, a.y + b.y, a.z + b.z)
}

/// Reflect `d` (incoming ray direction) around `n`.
#[inline]
fn reflect(d: Vec3, n: Vec3) -> Vec3 {
    let dot2 = 2.0 * (d.x * n.x + d.y * n.y + d.z * n.z);
    Vec3::new(d.x - dot2 * n.x, d.y - dot2 * n.y, d.z - dot2 * n.z)
}

#[inline]
fn spectrum_mul(a: Spectrum, b: Spectrum) -> Spectrum {
    Spectrum::rgb(a.r * b.r, a.g * b.g, a.b * b.b)
}

#[inline]
fn luminance(s: Spectrum) -> f32 {
    (0.2126 * s.r + 0.7152 * s.g + 0.0722 * s.b).clamp(0.0, 1.0)
}

#[inline]
fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

#[inline]
fn lerp_spectrum(a: Spectrum, b: Spectrum, t: f32) -> Spectrum {
    let t = t.clamp(0.0, 1.0);
    Spectrum::rgb(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
    )
}

#[inline]
fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn spectrum_to_rgb8_reinhard(s: Spectrum) -> [u8; 3] {
    fn tone(v: f32) -> u8 {
        let x = v.max(0.0);
        let mapped = (x / (1.0 + x)).powf(1.0 / 2.2).min(1.0);
        (mapped * 255.0) as u8
    }
    [tone(s.r), tone(s.g), tone(s.b)]
}

fn report_material_distribution(lanes: &TerrainMaterialLanes<'_>, world_size: f32) {
    if let Some(bundle) = lanes.bundle.as_ref() {
        let n = 128u32;
        let mut sum_mask = 0.0f32;
        let mut cov35 = 0u32;
        let mut cov50 = 0u32;
        let mut sum_r = 0.0f32;
        let mut sum_g = 0.0f32;
        let mut sum_b = 0.0f32;
        for z in 0..n {
            for x in 0..n {
                let fx = (x as f32 + 0.5) / n as f32 * world_size;
                let fz = (z as f32 + 0.5) / n as f32 * world_size;
                let p = Vec3::new(fx, 0.5, fz);
                let nn = Vec3::new(0.0, 1.0, 0.0);
                let vd = Vec3::new(0.0, 0.0, -1.0);
                let m = bundle.displacement(p, nn, vd).abs().clamp(0.0, 1.0);
                sum_mask += m;
                if m >= 0.35 {
                    cov35 += 1;
                }
                if m >= 0.50 {
                    cov50 += 1;
                }
                let c = bundle.base_color(p, nn, vd);
                sum_r += c.r;
                sum_g += c.g;
                sum_b += c.b;
            }
        }
        let total = (n * n) as f32;
        eprintln!(
            "[terrain-material] mask_mean={:.3} cov>=0.35={:.1}% cov>=0.50={:.1}% base_rgb_mean=({:.3},{:.3},{:.3})",
            sum_mask / total,
            cov35 as f32 * 100.0 / total,
            cov50 as f32 * 100.0 / total,
            sum_r / total,
            sum_g / total,
            sum_b / total,
        );
        return;
    }

    eprintln!("[terrain-material] no material bundle bound");
}

fn parse_color_value(value: &Value) -> Option<Spectrum> {
    match value {
        Value::String(s) => parse_hex_color(s),
        Value::Array(items) if items.len() == 3 => {
            let r = match &items[0] {
                Value::Number(v) => *v,
                _ => return None,
            };
            let g = match &items[1] {
                Value::Number(v) => *v,
                _ => return None,
            };
            let b = match &items[2] {
                Value::Number(v) => *v,
                _ => return None,
            };
            Some(Spectrum::rgb(r, g, b))
        }
        Value::Object(_) | Value::Number(_) => value_to_spectrum(value),
        _ => None,
    }
}

fn parse_hex_color(s: &str) -> Option<Spectrum> {
    let hex = s.strip_prefix('#')?;
    let (r, g, b) = match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            (r, g, b)
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            (r, g, b)
        }
        _ => return None,
    };
    Some(Spectrum::rgb(
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
    ))
}

fn value_to_spectrum(value: &Value) -> Option<Spectrum> {
    match value {
        Value::Object(obj) => {
            let x = match obj.fields.get("x")? {
                Value::Number(v) => *v,
                _ => return None,
            };
            let y = match obj.fields.get("y")? {
                Value::Number(v) => *v,
                _ => return None,
            };
            let z = match obj.fields.get("z")? {
                Value::Number(v) => *v,
                _ => return None,
            };
            Some(Spectrum::rgb(x, y, z))
        }
        Value::Number(v) => Some(Spectrum::rgb(*v, *v, *v)),
        _ => None,
    }
}
