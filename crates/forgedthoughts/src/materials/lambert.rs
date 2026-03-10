use std::f32::consts::PI;

use crate::render_api::{Spectrum, Vec3};

use super::{BsdfSample, LambertMaterial, SampleInput};

pub fn eval(material: LambertMaterial, normal: Vec3, wi: Vec3, _wo: Vec3) -> Spectrum {
    if normal.dot(wi) <= 0.0 {
        Spectrum::black()
    } else {
        material.color.scale(1.0 / PI)
    }
}

pub fn pdf(_material: LambertMaterial, normal: Vec3, wi: Vec3, _wo: Vec3) -> f32 {
    normal.dot(wi).max(0.0) / PI
}

pub fn sample(
    material: LambertMaterial,
    normal: Vec3,
    _wo: Vec3,
    input: SampleInput,
) -> BsdfSample {
    let wi = cosine_sample_hemisphere(normal, input.u1, input.u2);
    let f = eval(material, normal, wi, wi);
    let pdf = pdf(material, normal, wi, wi).max(1.0e-6);
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

fn cosine_sample_hemisphere(normal: Vec3, u1: f32, u2: f32) -> Vec3 {
    let uu1 = u1.clamp(1.0e-6, 1.0 - 1.0e-6);
    let uu2 = u2.clamp(1.0e-6, 1.0 - 1.0e-6);
    let r = uu1.sqrt();
    let phi = 2.0 * PI * uu2;
    let x = r * phi.cos();
    let y = r * phi.sin();
    let z = (1.0 - uu1).sqrt();
    to_world(normal, x, y, z)
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
