use super::*;

pub(super) fn trace_ray_recursive(
    accel: &(impl Accelerator + Sync),
    setup: &RenderSetup,
    ctx: RayTraceCtx,
    origin: Vec3,
    dir: Vec3,
    medium: MediumState,
    depth: u32,
) -> Spectrum {
    if depth >= ctx.max_depth {
        return Spectrum::black();
    }

    let min_t = if depth == 0 {
        0.0
    } else {
        secondary_min_t(ctx.options.epsilon)
    };
    let Some(hit) = raymarch_hit(accel, origin, dir, ctx.options, min_t, ctx.options.max_dist)
    else {
        return apply_medium_attenuation(
            environment_color(setup, dir).unwrap_or_else(|| env_radiance(&setup.path_lights)),
            medium,
            ctx.options.max_dist,
        );
    };

    let view_dir = dir.mul(-1.0).normalize();
    if let Some((a, b, t)) = resolve_split_material_at_hit(setup, hit, view_dir) {
        let a = trace_hit_with_material(accel, setup, ctx, hit, dir, medium, depth, a);
        let b = trace_hit_with_material(accel, setup, ctx, hit, dir, medium, depth, b);
        return lerp_spectrum(a, b, t);
    }
    let mat = resolve_material_at_hit(setup, hit, view_dir);
    trace_hit_with_material(accel, setup, ctx, hit, dir, medium, depth, mat)
}

#[allow(clippy::too_many_arguments)]
fn trace_hit_with_material(
    accel: &(impl Accelerator + Sync),
    setup: &RenderSetup,
    ctx: RayTraceCtx,
    hit: RayHit,
    dir: Vec3,
    medium: MediumState,
    depth: u32,
    mat: MaterialKindRt,
) -> Spectrum {
    let hit_point = hit.position;
    let normal = if hit.front_face {
        hit.normal.normalize()
    } else {
        hit.normal.mul(-1.0).normalize()
    };
    let bsdf_ctx = build_bsdf_context(setup, hit, dir.mul(-1.0), medium.ior);
    let params = dominant_material_params(mat);
    let roughness = params.roughness.clamp(0.0, 1.0);
    let (
        transmission,
        ior,
        thin,
        specular_color,
        base_color,
        metallic,
        clearcoat,
        specular_strength,
    ) = match dominant_material_model(mat) {
        MaterialKindTag::Standard => (
            params.transmission.clamp(0.0, 1.0),
            params.ior.clamp(1.0, 3.0),
            params.thin_walled,
            params.specular_color,
            params.color,
            params.metallic.clamp(0.0, 1.0),
            params.clearcoat.clamp(0.0, 1.0),
            (params.specular * params.specular_weight).clamp(0.0, 1.0),
        ),
        MaterialKindTag::Lambert => (
            0.0,
            1.0,
            false,
            Spectrum::rgb(0.0, 0.0, 0.0),
            params.color,
            0.0,
            0.0,
            0.0,
        ),
        MaterialKindTag::Metal => (0.0, 1.0, false, params.color, params.color, 1.0, 0.0, 1.0),
        MaterialKindTag::Dielectric => (
            1.0,
            params.ior.clamp(1.0, 3.0),
            params.thin_walled,
            Spectrum::rgb(1.0, 1.0, 1.0),
            params.color,
            0.0,
            0.0,
            1.0,
        ),
    };
    let (eta_i, eta_t, next_ior) = if thin {
        (1.0, 1.0, medium.ior)
    } else if hit.front_face {
        (medium.ior, ior, ior)
    } else {
        (medium.ior, 1.0, 1.0)
    };

    let wo = bsdf_ctx.wo;
    let geometric_normal = if hit.front_face {
        hit.normal.normalize()
    } else {
        hit.normal.mul(-1.0).normalize()
    };
    let fresnel = if transmission > 1.0e-4 {
        fresnel_dielectric_scalar(normal.dot(wo).abs(), eta_i, eta_t)
    } else {
        let f0 = fresnel_f0_from_ior(ior).clamp(0.0, 1.0);
        fresnel_schlick_scalar(f0, normal.dot(wo).abs()).clamp(0.0, 1.0)
    };
    let reflect_weight = if transmission > 1.0e-4 {
        fresnel.clamp(0.0, 1.0)
    } else {
        let base_reflect = if metallic > 0.5 {
            1.0
        } else {
            specular_strength.clamp(0.0, 1.0)
        };
        (base_reflect * fresnel + metallic * 0.8 + clearcoat * 0.2).clamp(0.0, 1.0)
    };

    let local_weight = ((1.0 - transmission) * (1.0 - reflect_weight)).clamp(0.0, 1.0);
    let mut color =
        shade_color(accel, setup, ctx.options, &setup.lights, mat, bsdf_ctx).scale(local_weight);
    let emission = mat.emission();
    color = color + emission;

    if reflect_weight > 1.0e-4 {
        let reflected = if transmission > 1.0e-4 && roughness > 0.06 {
            trace_rough_dielectric_reflection(
                accel,
                setup,
                ctx,
                hit_point,
                geometric_normal,
                dir,
                normal,
                medium,
                depth,
                roughness,
            )
        } else {
            let reflect_dir = reflect(dir, normal).normalize();
            let reflect_origin = offset_ray_origin(
                hit_point,
                geometric_normal,
                reflect_dir,
                ctx.options.epsilon * (1.0 + roughness * 6.0),
            );
            trace_ray_recursive(
                accel,
                setup,
                ctx,
                reflect_origin,
                reflect_dir,
                medium,
                depth + 1,
            )
        };
        let reflect_tint = lerp_spectrum(specular_color, base_color, metallic.clamp(0.0, 1.0));
        color = color + (reflected * reflect_tint).scale(reflect_weight);
    }

    if transmission > 1.0e-4 {
        let mut next_medium = if thin {
            medium
        } else {
            transition_medium(mat, hit.front_face, medium)
        };
        next_medium.ior = next_ior.clamp(1.0, 3.0);
        let refracted = if roughness > 0.06 {
            trace_rough_dielectric_refraction(
                accel,
                setup,
                ctx,
                hit_point,
                geometric_normal,
                dir,
                normal,
                eta_i / eta_t,
                next_medium,
                depth,
                roughness,
            )
        } else if let Some(refract_dir) = refract(dir, normal, eta_i / eta_t) {
            let refract_origin = offset_ray_origin(
                hit_point,
                geometric_normal,
                refract_dir,
                ctx.options.epsilon * (8.0 + roughness * 12.0),
            );
            trace_ray_recursive(
                accel,
                setup,
                ctx,
                refract_origin,
                refract_dir.normalize(),
                next_medium,
                depth + 1,
            )
        } else {
            Spectrum::black()
        };
        if refracted != Spectrum::black() {
            let trans_tint = transmission_tint(base_color, transmission);
            color = color + (refracted * trans_tint).scale(transmission * (1.0 - fresnel));
        }
    }

    apply_medium_attenuation(color, medium, hit.t)
}

pub(super) fn trace_ray_debug_aov(
    accel: &(impl Accelerator + Sync),
    setup: &RenderSetup,
    ctx: RayTraceCtx,
    origin: Vec3,
    dir: Vec3,
    aov: RayDebugAov,
) -> Spectrum {
    let Some(hit) = raymarch_hit(accel, origin, dir, ctx.options, 0.0, ctx.options.max_dist) else {
        return Spectrum::black();
    };
    let mat = resolve_material_at_hit(setup, hit, dir.mul(-1.0).normalize());
    let normal = if hit.front_face {
        hit.normal.normalize()
    } else {
        hit.normal.mul(-1.0).normalize()
    };
    match aov {
        RayDebugAov::Depth => {
            let d = (1.0 - (hit.t / ctx.options.max_dist)).clamp(0.0, 1.0);
            Spectrum::rgb(d, d, d)
        }
        RayDebugAov::Normal => Spectrum::rgb(
            normal.x * 0.5 + 0.5,
            normal.y * 0.5 + 0.5,
            normal.z * 0.5 + 0.5,
        ),
        RayDebugAov::MaterialId => material_id_color(hit.material_id),
        RayDebugAov::Ior => {
            let ior = match dominant_material_model(mat) {
                MaterialKindTag::Dielectric | MaterialKindTag::Standard => {
                    dominant_material_params(mat).ior.clamp(1.0, 3.0)
                }
                _ => 1.0,
            };
            let v = ((ior - 1.0) / 2.0).clamp(0.0, 1.0);
            Spectrum::rgb(v, v, v)
        }
        RayDebugAov::Transmission => {
            let v = matches!(
                dominant_material_model(mat),
                MaterialKindTag::Dielectric | MaterialKindTag::Standard
            ) as u8 as f32;
            Spectrum::rgb(v, v, v)
        }
        RayDebugAov::Fresnel => {
            let wo = dir.mul(-1.0).normalize();
            let ior = match dominant_material_model(mat) {
                MaterialKindTag::Dielectric | MaterialKindTag::Standard => {
                    dominant_material_params(mat).ior.clamp(1.0, 3.0)
                }
                _ => 1.0,
            };
            let f = fresnel_dielectric_scalar(normal.dot(wo).abs(), 1.0, ior);
            Spectrum::rgb(f, f, f)
        }
        RayDebugAov::HitT => {
            let v = (hit.t / ctx.options.max_dist).clamp(0.0, 1.0);
            Spectrum::rgb(v, v, v)
        }
    }
}

fn transmission_tint(base_color: Spectrum, transmission: f32) -> Spectrum {
    let t = transmission.clamp(0.0, 1.0);
    Spectrum::rgb(
        1.0 - (1.0 - base_color.r.clamp(0.0, 1.0)) * t,
        1.0 - (1.0 - base_color.g.clamp(0.0, 1.0)) * t,
        1.0 - (1.0 - base_color.b.clamp(0.0, 1.0)) * t,
    )
}

#[allow(clippy::too_many_arguments)]
fn trace_rough_dielectric_reflection(
    accel: &(impl Accelerator + Sync),
    setup: &RenderSetup,
    ctx: RayTraceCtx,
    hit_point: Vec3,
    geometric_normal: Vec3,
    dir: Vec3,
    normal: Vec3,
    medium: MediumState,
    depth: u32,
    roughness: f32,
) -> Spectrum {
    let ideal = reflect(dir, normal).normalize();
    trace_lobe_average(
        accel,
        setup,
        ctx,
        hit_point,
        geometric_normal,
        ideal,
        medium,
        depth,
        roughness,
        rough_dielectric_reflection_samples(roughness),
    )
}

#[allow(clippy::too_many_arguments)]
fn trace_rough_dielectric_refraction(
    accel: &(impl Accelerator + Sync),
    setup: &RenderSetup,
    ctx: RayTraceCtx,
    hit_point: Vec3,
    geometric_normal: Vec3,
    dir: Vec3,
    normal: Vec3,
    eta: f32,
    medium: MediumState,
    depth: u32,
    roughness: f32,
) -> Spectrum {
    let Some(ideal) = refract(dir, normal, eta) else {
        return Spectrum::black();
    };
    trace_lobe_average(
        accel,
        setup,
        ctx,
        hit_point,
        geometric_normal,
        ideal.normalize(),
        medium,
        depth,
        roughness,
        rough_dielectric_refraction_samples(roughness),
    )
}

#[allow(clippy::too_many_arguments)]
fn trace_lobe_average(
    accel: &(impl Accelerator + Sync),
    setup: &RenderSetup,
    ctx: RayTraceCtx,
    hit_point: Vec3,
    geometric_normal: Vec3,
    ideal_dir: Vec3,
    medium: MediumState,
    depth: u32,
    roughness: f32,
    sample_count: u32,
) -> Spectrum {
    let dirs = rough_lobe_directions(ideal_dir, roughness, sample_count);
    let inv = 1.0 / dirs.len() as f32;
    let mut sum = Spectrum::black();
    for d in dirs {
        let origin = offset_ray_origin(
            hit_point,
            geometric_normal,
            d,
            ctx.options.epsilon * (8.0 + roughness * 12.0),
        );
        sum = sum + trace_ray_recursive(accel, setup, ctx, origin, d, medium, depth + 1).scale(inv);
    }
    sum
}

fn rough_lobe_directions(ideal_dir: Vec3, roughness: f32, sample_count: u32) -> Vec<Vec3> {
    let ideal = ideal_dir.normalize();
    let tangent_seed = if ideal.y.abs() < 0.99 {
        Vec3::new(0.0, 1.0, 0.0)
    } else {
        Vec3::new(1.0, 0.0, 0.0)
    };
    let tangent = ideal.cross(tangent_seed).normalize();
    let bitangent = tangent.cross(ideal).normalize();
    let spread = roughness.clamp(0.0, 0.5) * 0.35;
    let mut dirs = Vec::with_capacity(sample_count.max(1) as usize);
    dirs.push(ideal);
    if sample_count <= 1 || spread <= 1.0e-4 {
        return dirs;
    }

    let offsets = [
        (1.0, 0.0),
        (-1.0, 0.0),
        (0.0, 1.0),
        (0.0, -1.0),
        (0.707, 0.707),
        (-0.707, 0.707),
    ];
    for &(ox, oy) in offsets.iter().take(sample_count.saturating_sub(1) as usize) {
        let d = ideal
            .add(tangent.mul(ox * spread))
            .add(bitangent.mul(oy * spread))
            .normalize();
        dirs.push(d);
    }
    dirs
}

fn rough_dielectric_reflection_samples(roughness: f32) -> u32 {
    if roughness < 0.12 {
        1
    } else if roughness < 0.28 {
        2
    } else {
        3
    }
}

fn rough_dielectric_refraction_samples(roughness: f32) -> u32 {
    if roughness < 0.1 {
        2
    } else if roughness < 0.24 {
        3
    } else {
        4
    }
}
