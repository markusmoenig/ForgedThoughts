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
    let (transmission, ior, thin, specular_color, base_color, metallic) =
        match dominant_material_model(mat) {
            MaterialKindTag::Lambert => (
                0.0,
                1.0,
                false,
                Spectrum::rgb(0.0, 0.0, 0.0),
                params.color,
                0.0,
            ),
            MaterialKindTag::Metal => (0.0, 1.0, false, params.color, params.color, 1.0),
            MaterialKindTag::Dielectric => (
                1.0,
                params.ior.clamp(1.0, 3.0),
                params.thin_walled,
                Spectrum::rgb(1.0, 1.0, 1.0),
                params.color,
                0.0,
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
    let reflect_dir = reflect(dir, normal).normalize();
    let geometric_normal = if hit.front_face {
        hit.normal.normalize()
    } else {
        hit.normal.mul(-1.0).normalize()
    };
    let reflect_origin = offset_ray_origin(
        hit_point,
        geometric_normal,
        reflect_dir,
        ctx.options.epsilon,
    );

    let fresnel = if transmission > 1.0e-4 {
        fresnel_dielectric_scalar(normal.dot(wo).abs(), eta_i, eta_t)
    } else {
        let f0 = fresnel_f0_from_ior(ior).clamp(0.0, 1.0);
        fresnel_schlick_scalar(f0, normal.dot(wo).abs()).clamp(0.0, 1.0)
    };
    let reflect_weight = if transmission > 1.0e-4 {
        fresnel.clamp(0.0, 1.0)
    } else {
        ((if metallic > 0.5 { 1.0 } else { 0.08 }) * fresnel + metallic * 0.8).clamp(0.0, 1.0)
    };

    let local_weight = ((1.0 - transmission) * (1.0 - reflect_weight)).clamp(0.0, 1.0);
    let mut color =
        shade_color(accel, setup, ctx.options, &setup.lights, mat, bsdf_ctx).scale(local_weight);
    let emission = mat.emission();
    color = color + emission;

    if reflect_weight > 1.0e-4 {
        let reflected = trace_ray_recursive(
            accel,
            setup,
            ctx,
            reflect_origin,
            reflect_dir,
            medium,
            depth + 1,
        );
        let reflect_tint = lerp_spectrum(specular_color, base_color, metallic.clamp(0.0, 1.0));
        color = color + (reflected * reflect_tint).scale(reflect_weight);
    }

    if transmission > 1.0e-4
        && let Some(refract_dir) = refract(dir, normal, eta_i / eta_t)
    {
        let refract_origin = offset_ray_origin(
            hit_point,
            geometric_normal,
            refract_dir,
            ctx.options.epsilon * 8.0,
        );
        let mut next_medium = if thin {
            medium
        } else {
            transition_medium(mat, hit.front_face, medium)
        };
        next_medium.ior = next_ior.clamp(1.0, 3.0);
        let refracted = trace_ray_recursive(
            accel,
            setup,
            ctx,
            refract_origin,
            refract_dir.normalize(),
            next_medium,
            depth + 1,
        );
        let trans_tint = lerp_spectrum(Spectrum::rgb(1.0, 1.0, 1.0), base_color, transmission);
        color = color + (refracted * trans_tint).scale(transmission * (1.0 - fresnel));
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
                MaterialKindTag::Dielectric => dominant_material_params(mat).ior.clamp(1.0, 3.0),
                _ => 1.0,
            };
            let v = ((ior - 1.0) / 2.0).clamp(0.0, 1.0);
            Spectrum::rgb(v, v, v)
        }
        RayDebugAov::Transmission => {
            let v =
                matches!(dominant_material_model(mat), MaterialKindTag::Dielectric) as u8 as f32;
            Spectrum::rgb(v, v, v)
        }
        RayDebugAov::Fresnel => {
            let wo = dir.mul(-1.0).normalize();
            let ior = match dominant_material_model(mat) {
                MaterialKindTag::Dielectric => dominant_material_params(mat).ior.clamp(1.0, 3.0),
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
