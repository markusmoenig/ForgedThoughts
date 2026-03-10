use crate::render_api::{Spectrum, Vec3};

use super::{BsdfSample, DielectricMaterial, SampleInput};

pub fn eval(_material: DielectricMaterial, _normal: Vec3, _wi: Vec3, _wo: Vec3) -> Spectrum {
    Spectrum::black()
}

pub fn pdf(_material: DielectricMaterial, _normal: Vec3, _wi: Vec3, _wo: Vec3) -> f32 {
    0.0
}

pub fn sample(
    material: DielectricMaterial,
    normal: Vec3,
    wo: Vec3,
    input: SampleInput,
) -> BsdfSample {
    let ior = material.ior.clamp(1.0, 3.0);
    let (eta_i, eta_t, next_ior) = if material.thin_walled {
        (1.0, 1.0, input.current_ior)
    } else if input.front_face {
        (input.current_ior, ior, ior)
    } else {
        (input.current_ior, 1.0, 1.0)
    };
    let n = normal.normalize();
    let incident = wo * -1.0;
    let fresnel = fresnel_dielectric_scalar(n.dot(wo).abs(), eta_i, eta_t).clamp(0.0, 1.0);
    if input.u1 < fresnel {
        let wi = reflect(incident, n).normalize();
        BsdfSample {
            wi,
            f: Spectrum::rgb(1.0, 1.0, 1.0),
            pdf: fresnel.max(1.0e-6),
            delta: true,
            apply_cos: false,
            transmission: false,
            thin_walled: material.thin_walled,
            next_ior: input.current_ior,
        }
    } else if let Some(wi) = refract(incident, n, eta_i / eta_t) {
        BsdfSample {
            wi: wi.normalize(),
            f: material.color,
            pdf: (1.0 - fresnel).max(1.0e-6),
            delta: true,
            apply_cos: false,
            transmission: true,
            thin_walled: material.thin_walled,
            next_ior,
        }
    } else {
        let wi = reflect(incident, n).normalize();
        BsdfSample {
            wi,
            f: Spectrum::rgb(1.0, 1.0, 1.0),
            pdf: 1.0,
            delta: true,
            apply_cos: false,
            transmission: false,
            thin_walled: material.thin_walled,
            next_ior: input.current_ior,
        }
    }
}

fn reflect(v: Vec3, n: Vec3) -> Vec3 {
    v - n * (2.0 * v.dot(n))
}

fn refract(incident: Vec3, normal: Vec3, eta: f32) -> Option<Vec3> {
    let i = incident.normalize();
    let mut n = normal.normalize();
    let mut cosi = i.dot(n).clamp(-1.0, 1.0);
    if cosi > 0.0 {
        n = n * -1.0;
    } else {
        cosi = -cosi;
    }
    let sin2_t = eta * eta * (1.0 - cosi * cosi);
    if sin2_t > 1.0 {
        return None;
    }
    let cost = (1.0 - sin2_t).sqrt();
    Some((i * eta + n * (eta * cosi - cost)).normalize())
}

fn fresnel_dielectric_scalar(cos_theta_i: f32, eta_i: f32, eta_t: f32) -> f32 {
    let ei = eta_i.max(1.0e-4);
    let et = eta_t.max(1.0e-4);
    if (ei - et).abs() < 1.0e-6 {
        return 0.0;
    }
    let ci = cos_theta_i.clamp(0.0, 1.0);
    let eta = ei / et;
    let sin2_t = eta * eta * (1.0 - ci * ci);
    if sin2_t >= 1.0 {
        return 1.0;
    }
    let ct = (1.0 - sin2_t).sqrt();
    let rs = ((ei * ci) - (et * ct)) / ((ei * ci) + (et * ct)).max(1.0e-6);
    let rp = ((et * ci) - (ei * ct)) / ((et * ci) + (ei * ct)).max(1.0e-6);
    0.5 * (rs * rs + rp * rp)
}
