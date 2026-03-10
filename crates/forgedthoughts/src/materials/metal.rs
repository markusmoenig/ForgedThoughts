use std::f32::consts::PI;

use crate::render_api::{Spectrum, Vec3};

use super::{BsdfSample, MetalMaterial, SampleInput};

pub fn eval(material: MetalMaterial, normal: Vec3, wi: Vec3, wo: Vec3) -> Spectrum {
    let ndotl = normal.dot(wi).max(0.0);
    let ndotv = normal.dot(wo).max(0.0);
    if ndotl <= 0.0 || ndotv <= 0.0 {
        return Spectrum::black();
    }
    let h = (wi + wo).normalize();
    let ndoth = normal.dot(h).max(0.0);
    let vdoth = wo.dot(h).max(0.0);
    let alpha = material.roughness.clamp(0.02, 1.0).powi(2);
    let d = ggx_d(ndoth, alpha);
    let g = smith_ggx_g(ndotl, ndotv, alpha);
    let fresnel = fresnel_schlick(material.color, vdoth);
    fresnel.scale((d * g) / (4.0 * ndotl * ndotv).max(1.0e-5))
}

pub fn pdf(material: MetalMaterial, normal: Vec3, wi: Vec3, wo: Vec3) -> f32 {
    if normal.dot(wi) <= 0.0 || normal.dot(wo) <= 0.0 {
        return 0.0;
    }
    let h = (wi + wo).normalize();
    let ndoth = normal.dot(h).max(0.0);
    let vdoth = wo.dot(h).abs().max(1.0e-6);
    let alpha = material.roughness.clamp(0.02, 1.0).powi(2);
    let d = ggx_d(ndoth, alpha);
    (d * ndoth / (4.0 * vdoth)).max(0.0)
}

pub fn sample(material: MetalMaterial, normal: Vec3, wo: Vec3, input: SampleInput) -> BsdfSample {
    let wi = sample_ggx_reflection(normal, wo, material.roughness, input.u1, input.u2);
    let f = eval(material, normal, wi, wo);
    let pdf = pdf(material, normal, wi, wo).max(1.0e-6);
    BsdfSample {
        wi,
        f,
        pdf,
        delta: false,
        apply_cos: true,
        transmission: false,
        thin_walled: false,
        next_ior: input.current_ior,
    }
}

fn sample_ggx_reflection(normal: Vec3, wo: Vec3, roughness: f32, u1: f32, u2: f32) -> Vec3 {
    let alpha = roughness.clamp(0.02, 1.0).powi(2);
    let uu1 = u1.clamp(1.0e-6, 1.0 - 1.0e-6);
    let uu2 = u2.clamp(1.0e-6, 1.0 - 1.0e-6);
    let phi = 2.0 * PI * uu2;
    let cos_theta = ((1.0 - uu1) / (1.0 + (alpha * alpha - 1.0) * uu1)).sqrt();
    let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
    let h = to_world(
        normal,
        sin_theta * phi.cos(),
        sin_theta * phi.sin(),
        cos_theta,
    );
    reflect(wo * -1.0, h).normalize()
}

fn to_world(normal: Vec3, x: f32, y: f32, z: f32) -> Vec3 {
    let n = normal.normalize();
    let helper = if n.y.abs() < 0.99 {
        Vec3::new(0.0, 1.0, 0.0)
    } else {
        Vec3::new(1.0, 0.0, 0.0)
    };
    let t = helper.cross(n).normalize();
    let b = n.cross(t).normalize();
    (t * x + b * y + n * z).normalize()
}

fn reflect(v: Vec3, n: Vec3) -> Vec3 {
    v - n * (2.0 * v.dot(n))
}

fn fresnel_schlick(f0: Spectrum, cos_theta: f32) -> Spectrum {
    let m = (1.0 - cos_theta.clamp(0.0, 1.0)).powi(5);
    f0 + Spectrum::rgb(1.0, 1.0, 1.0).scale(m) + f0.scale(-m)
}

fn ggx_d(ndoth: f32, alpha: f32) -> f32 {
    let a2 = alpha * alpha;
    let n2 = ndoth * ndoth;
    let denom = (n2 * (a2 - 1.0) + 1.0).max(1.0e-5);
    a2 / (PI * denom * denom)
}

fn smith_ggx_g(ndotl: f32, ndotv: f32, alpha: f32) -> f32 {
    smith_ggx_g1(ndotl, alpha) * smith_ggx_g1(ndotv, alpha)
}

fn smith_ggx_g1(ndotx: f32, alpha: f32) -> f32 {
    let k = (alpha * alpha) * 0.5;
    ndotx / (ndotx * (1.0 - k) + k).max(1.0e-5)
}
