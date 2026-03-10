use super::*;

pub(super) fn trace_path(
    accel: &(impl Accelerator + Sync),
    setup: &RenderSetup,
    mut origin: Vec3,
    mut dir: Vec3,
    options: RenderOptions,
    max_bounces: u32,
    rng: &mut XorShift64,
) -> Spectrum {
    let mut throughput = Spectrum::rgb(1.0, 1.0, 1.0);
    let mut radiance = Spectrum::black();
    let direct_ctx = DirectLightingCtx {
        accel,
        setup,
        options,
        lights: &setup.path_lights,
    };
    let mut prev_env_mis_pdf: Option<f32> = None;
    let mut prev_env_mis_ctx: Option<BsdfContextBase> = None;
    let mut prev_env_mis_mat: Option<MaterialKindRt> = None;
    let mut medium = MediumState::air();

    for bounce in 0..max_bounces {
        let Some(hit) = raymarch_hit(accel, origin, dir, options, 0.0, options.max_dist) else {
            let env = apply_medium_attenuation(
                env_radiance(&setup.path_lights),
                medium,
                options.max_dist,
            );
            let weight = if let (Some(bsdf_pdf), Some(prev_ctx), Some(prev_mat)) =
                (prev_env_mis_pdf, prev_env_mis_ctx, prev_env_mis_mat)
            {
                let light_pdf =
                    env_light_pdf_for_dir(&setup.path_lights, setup, prev_mat, prev_ctx, dir)
                        .max(1.0e-6);
                power_heuristic(bsdf_pdf.max(1.0e-6), light_pdf)
            } else {
                1.0
            };
            radiance = radiance + (throughput * env).scale(weight);
            break;
        };

        throughput = throughput * medium_transmittance(medium, hit.t);
        let hit_point = hit.position;
        let _ = (hit.object_id, hit.material_id);
        let mat = resolve_material_at_hit(setup, hit, dir.mul(-1.0).normalize());
        let normal = if hit.front_face {
            hit.normal.normalize()
        } else {
            hit.normal.mul(-1.0).normalize()
        };
        let wo = dir.mul(-1.0).normalize();
        let bsdf_ctx = BsdfContextBase {
            hit,
            local_position: to_local(
                hit.position,
                setup
                    .object_transforms
                    .get(hit.object_id as usize)
                    .copied()
                    .unwrap_or_else(PrimitiveTransform::identity),
            ),
            normal,
            wo,
            current_ior: medium.ior,
        };
        let emission = mat.emission();
        if emission.r > 0.0 || emission.g > 0.0 || emission.b > 0.0 {
            radiance = radiance + (throughput * emission);
        }

        let direct = estimate_direct_mis(&direct_ctx, mat, hit_point, bsdf_ctx, rng);
        radiance = radiance + clamp_spectrum(throughput * direct, 12.0);

        let sample = sample_bsdf_lobe(setup, mat, bsdf_ctx, rng);
        if sample.pdf <= 1.0e-6 {
            break;
        }
        if sample.apply_cos {
            let cos_theta = normal.dot(sample.wi).max(0.0);
            if cos_theta <= 0.0 {
                break;
            }
            throughput = throughput * sample.f.scale(cos_theta / sample.pdf.max(1.0e-6));
        } else {
            throughput = throughput * sample.f.scale(1.0 / sample.pdf.max(1.0e-6));
        }
        throughput = clamp_spectrum(throughput, 8.0);
        if sample.delta {
            prev_env_mis_pdf = None;
            prev_env_mis_ctx = None;
            prev_env_mis_mat = None;
        } else {
            prev_env_mis_pdf = Some(sample.pdf.max(1.0e-6));
            prev_env_mis_ctx = Some(bsdf_ctx);
            prev_env_mis_mat = Some(mat);
        }

        if bounce >= 2 {
            let p = spectrum_luminance(throughput).clamp(0.05, 0.98);
            if rng.next_f32() > p {
                break;
            }
            throughput = throughput.scale(1.0 / p);
        }

        dir = sample.wi.normalize();
        if sample.transmission && !sample.thin_walled {
            medium = transition_medium(mat, hit.front_face, medium);
            medium.ior = sample.next_ior.clamp(1.0, 3.0);
            origin = hit_point.add(dir.mul((options.epsilon * 256.0).max(1.0e-3)));
        } else {
            origin = hit_point.add(normal.mul((options.epsilon * 10.0).max(1.0e-4)));
        }
    }

    radiance
}
