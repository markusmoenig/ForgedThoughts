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

use std::sync::mpsc;
use std::time::Instant;

use image::{ImageError, RgbImage};
use rayon::prelude::*;

use crate::{
    EvalState, GraphCamera, GraphRenderSourceKind, GraphSceneSettings, GraphSky, GraphSun,
    Value, compile_specialized_height_node_eval_function,
    render_api::{PinholeCamera, Vec3, Camera, Spectrum},
};

use super::{RenderError, RenderProgress};
use super::height_source::HeightSampler;

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
    mut progress_cb: F,
) -> Result<RgbImage, RenderError>
where
    F: FnMut(RenderProgress, &RgbImage) -> Result<(), ImageError> + Send,
{
    // Find the graph binding.
    let graph_binding = state
        .bindings
        .get("graph")
        .ok_or(RenderError::MissingGraph)?;
    let graph_obj = match &graph_binding.value {
        Value::Object(obj) => obj.clone(),
        _ => return Err(RenderError::MissingGraph),
    };
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
    let sd_len = (sd[0]*sd[0] + sd[1]*sd[1] + sd[2]*sd[2]).sqrt().max(1e-6);
    let sun_dir = Vec3::new(sd[0]/sd_len, sd[1]/sd_len, sd[2]/sd_len);
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

    tile_indices.par_iter().for_each_with(sender, |s, &(tile_x, tile_y)| {
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
                    &sampler, compiled_eval.as_ref(),
                    ray.origin, ray.direction,
                    sun_dir, sun_color, sky_color,
                    scene,
                    0, // Whitted depth
                );

                let rgb = spectrum_to_rgb8_reinhard(color);
                let idx = (iy * tile_w + ix) * 3;
                pixels[idx]     = rgb[0];
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
                image.put_pixel(px, py, image::Rgb([pixels[idx], pixels[idx+1], pixels[idx+2]]));
            }
        }
        tiles_done += 1;
        if let Err(err) = progress_cb(
            RenderProgress { tiles_done, tiles_total, elapsed_ms: start.elapsed().as_millis() },
            &image,
        ) {
            return Err(RenderError::Image(err));
        }
    }

    Ok(image)
}

// ---------------------------------------------------------------------------
// Height sampling
// ---------------------------------------------------------------------------

fn sample_height(
    sampler: &HeightSampler<'_>,
    compiled: Option<&crate::jit::JitFunction>,
    scene: &GraphSceneSettings,
    x: f32,
    z: f32,
) -> f32 {
    let h = compiled
        .and_then(|jit| jit.invoke(&[x, z]))
        .or_else(|| sampler.sample_root_scalar(x, z))
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    h * scene.height_scale
}

// ---------------------------------------------------------------------------
// Raymarching
// ---------------------------------------------------------------------------

/// March a ray against the heightfield. Returns `(hit_point, normal)` or `None`.
fn march_terrain(
    sampler: &HeightSampler<'_>,
    compiled: Option<&crate::jit::JitFunction>,
    ro: Vec3,
    rd: Vec3,
    scene: &GraphSceneSettings,
) -> Option<(Vec3, Vec3)> {
    let max_dist = scene.max_dist;
    let eps      = scene.epsilon;
    let ws       = scene.world_size;
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

        let h = sample_height(sampler, compiled, scene, p.x, p.z);

        let diff = p.y - h;

        if diff < eps {
            // Refine to surface.
            let t_refined = (t - (eps - diff) * 0.5).max(0.0);
            let p_hit = vec3_fma(ro, rd, t_refined);
            let n = terrain_normal(sampler, compiled, scene, p_hit);
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
    scene: &GraphSceneSettings,
    p: Vec3,
    sun_dir: Vec3,
    n: Vec3,
) -> bool {
    // Offset origin along normal to avoid self-intersection.
    let offset = scene.normal_eps * 8.0;
    let origin = Vec3::new(
        p.x + n.x * offset,
        p.y + n.y * offset,
        p.z + n.z * offset,
    );
    march_terrain(sampler, compiled, origin, sun_dir, scene).is_none()
}

// ---------------------------------------------------------------------------
// Whitted integrator
// ---------------------------------------------------------------------------

/// Recursive Whitted shading. `depth` caps at 1 for one specular reflection bounce.
fn trace_terrain(
    sampler: &HeightSampler<'_>,
    compiled: Option<&crate::jit::JitFunction>,
    ro: Vec3,
    rd: Vec3,
    sun_dir: Vec3,
    sun_color: Spectrum,
    sky_color: Spectrum,
    scene: &GraphSceneSettings,
    depth: u32,
) -> Spectrum {
    match march_terrain(sampler, compiled, ro, rd, scene) {
        None => sky_radiance(rd, sun_dir, sky_color),
        Some((p, n)) => shade_hit(
            sampler, compiled,
            scene, p, n, rd,
            sun_dir, sun_color, sky_color,
            depth,
        ),
    }
}

fn shade_hit(
    sampler: &HeightSampler<'_>,
    compiled: Option<&crate::jit::JitFunction>,
    scene: &GraphSceneSettings,
    p: Vec3,
    n: Vec3,
    view_dir: Vec3,
    sun_dir: Vec3,
    sun_color: Spectrum,
    sky_color: Spectrum,
    depth: u32,
) -> Spectrum {
    // Albedo is neutral — appearance comes from the graph, not the renderer.
    let albedo = Spectrum::rgb(0.70, 0.68, 0.65);

    // --- Shadow ray (hard shadows) ---
    let lit = sun_visible(sampler, compiled, scene, p, sun_dir, n);

    // --- Direct illumination: Lambert diffuse ---
    let n_dot_l = n.dot(sun_dir).max(0.0);
    let direct = if lit {
        spectrum_mul(albedo, sun_color.scale(n_dot_l))
    } else {
        // In shadow: only low-frequency bounce light (no direct sun).
        spectrum_mul(albedo, sun_color.scale(n_dot_l * 0.05))
    };

    // --- Sky ambient (hemisphere approximation) ---
    // Flat surfaces (n.y ≈ 1) see the full sky dome; vertical faces see half.
    let sky_weight = 0.5 + 0.5 * n.y.clamp(0.0, 1.0);
    let ambient = spectrum_mul(albedo, sky_color.scale(sky_weight));

    // --- Specular: Whitted reflection ray with Fresnel-Schlick ---
    // F0 = 0.04 (dielectric rock/soil). One bounce only.
    let specular = if depth < 1 {
        let wo = vec3_neg(view_dir);
        let n_dot_v = n.dot(wo).max(0.0);
        let f0 = 0.04_f32;
        let fresnel = f0 + (1.0 - f0) * (1.0 - n_dot_v).powf(5.0);

        if fresnel > 1e-4 {
            let reflect_dir = reflect(view_dir, n);
            let offset = scene.normal_eps * 8.0;
            let reflect_origin = Vec3::new(
                p.x + n.x * offset,
                p.y + n.y * offset,
                p.z + n.z * offset,
            );
            let reflected = trace_terrain(
                sampler, compiled,
                reflect_origin, reflect_dir,
                sun_dir, sun_color, sky_color,
                scene,
                depth + 1,
            );
            reflected.scale(fresnel)
        } else {
            Spectrum::black()
        }
    } else {
        Spectrum::black()
    };

    direct + ambient + specular
}

// ---------------------------------------------------------------------------
// Normal estimation
// ---------------------------------------------------------------------------

fn terrain_normal(
    sampler: &HeightSampler<'_>,
    compiled: Option<&crate::jit::JitFunction>,
    scene: &GraphSceneSettings,
    p: Vec3,
) -> Vec3 {
    let e = scene.normal_eps;
    let h0 = sample_height(sampler, compiled, scene, p.x,     p.z    );
    let hx = sample_height(sampler, compiled, scene, p.x + e, p.z    );
    let hz = sample_height(sampler, compiled, scene, p.x,     p.z + e);
    vec3_normalize(Vec3::new(h0 - hx, e, h0 - hz))
}

// ---------------------------------------------------------------------------
// Sky
// ---------------------------------------------------------------------------

fn sky_radiance(rd: Vec3, sun_dir: Vec3, sky_color: Spectrum) -> Spectrum {
    let t = (rd.y * 0.5 + 0.5).clamp(0.0, 1.0);
    let horizon = Spectrum::rgb(0.72, 0.84, 0.96);
    let zenith  = Spectrum::rgb(0.18, 0.42, 0.78);
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
fn vec3_neg(v: Vec3) -> Vec3 { Vec3::new(-v.x, -v.y, -v.z) }

#[inline]
fn vec3_normalize(v: Vec3) -> Vec3 {
    let len = (v.x*v.x + v.y*v.y + v.z*v.z).sqrt().max(1e-6);
    Vec3::new(v.x/len, v.y/len, v.z/len)
}

/// Reflect `d` (incoming ray direction) around `n`.
#[inline]
fn reflect(d: Vec3, n: Vec3) -> Vec3 {
    let dot2 = 2.0 * (d.x*n.x + d.y*n.y + d.z*n.z);
    Vec3::new(d.x - dot2*n.x, d.y - dot2*n.y, d.z - dot2*n.z)
}

#[inline]
fn spectrum_mul(a: Spectrum, b: Spectrum) -> Spectrum {
    Spectrum::rgb(a.r*b.r, a.g*b.g, a.b*b.b)
}

#[inline]
fn lerp_spectrum(a: Spectrum, b: Spectrum, t: f32) -> Spectrum {
    let t = t.clamp(0.0, 1.0);
    Spectrum::rgb(a.r + (b.r-a.r)*t, a.g + (b.g-a.g)*t, a.b + (b.b-a.b)*t)
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
