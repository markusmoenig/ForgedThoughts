use std::f32::consts::PI;

use crate::render_api::{Spectrum, Vec3};

use super::{BsdfSample, MaterialParams, SampleInput, lambert, metal};

pub fn eval(material: MaterialParams, normal: Vec3, wi: Vec3, wo: Vec3) -> Spectrum {
    let ndotl = normal.dot(wi).max(0.0);
    let ndotv = normal.dot(wo).max(0.0);
    if ndotl <= 0.0 || ndotv <= 0.0 {
        return Spectrum::black();
    }

    let metallic = material.metallic.clamp(0.0, 1.0);
    let transmission = material.transmission.clamp(0.0, 1.0);
    let base_kd = ((1.0 - metallic) * (1.0 - transmission)).clamp(0.0, 1.0);
    let diffuse = material.color.scale(base_kd / PI);

    let f0_dielectric = fresnel_f0_from_ior(material.ior).clamp(0.0, 1.0);
    let base_f0 = Spectrum::rgb(f0_dielectric, f0_dielectric, f0_dielectric)
        .scale(material.specular.clamp(0.0, 1.0) * material.specular_weight.clamp(0.0, 1.0))
        * material.specular_color;
    let metal_f0 = material.color;
    let spec_f0 = lerp_spectrum(base_f0, metal_f0, metallic);
    let specular_params =
        MaterialParams::metal(spec_f0, material.roughness, Spectrum::black(), 0.0);
    let specular = metal::eval(specular_params, normal, wi, wo);

    let clearcoat = material.clearcoat.clamp(0.0, 1.0);
    let clearcoat_term = if clearcoat > 1.0e-5 {
        let clearcoat_params = MaterialParams::metal(
            Spectrum::rgb(1.0, 1.0, 1.0),
            material.clearcoat_roughness.clamp(0.02, 1.0),
            Spectrum::black(),
            0.0,
        );
        metal::eval(clearcoat_params, normal, wi, wo).scale(clearcoat * 0.25)
    } else {
        Spectrum::black()
    };

    diffuse + specular + clearcoat_term
}

pub fn pdf(material: MaterialParams, normal: Vec3, wi: Vec3, wo: Vec3) -> f32 {
    let metallic = material.metallic.clamp(0.0, 1.0);
    let transmission = material.transmission.clamp(0.0, 1.0);
    let clearcoat = material.clearcoat.clamp(0.0, 1.0);

    let diffuse_w = ((1.0 - metallic) * (1.0 - transmission) * (1.0 - clearcoat)).max(0.0);
    let specular_w =
        ((material.specular * material.specular_weight) * (1.0 - transmission)).clamp(0.0, 1.0);
    let clearcoat_w = clearcoat * 0.25;
    let sum = diffuse_w + specular_w + clearcoat_w;
    if sum <= 1.0e-6 {
        return lambert::pdf(material, normal, wi, wo);
    }

    let specular_params = MaterialParams::metal(
        Spectrum::rgb(1.0, 1.0, 1.0),
        material.roughness,
        Spectrum::black(),
        0.0,
    );
    let clearcoat_params = MaterialParams::metal(
        Spectrum::rgb(1.0, 1.0, 1.0),
        material.clearcoat_roughness.clamp(0.02, 1.0),
        Spectrum::black(),
        0.0,
    );

    (diffuse_w / sum) * lambert::pdf(material, normal, wi, wo)
        + (specular_w / sum) * metal::pdf(specular_params, normal, wi, wo)
        + (clearcoat_w / sum) * metal::pdf(clearcoat_params, normal, wi, wo)
}

pub fn sample(material: MaterialParams, normal: Vec3, wo: Vec3, input: SampleInput) -> BsdfSample {
    let metallic = material.metallic.clamp(0.0, 1.0);
    let transmission = material.transmission.clamp(0.0, 1.0);
    let clearcoat = material.clearcoat.clamp(0.0, 1.0);

    let diffuse_w = ((1.0 - metallic) * (1.0 - transmission) * (1.0 - clearcoat)).max(0.0);
    let specular_w =
        ((material.specular * material.specular_weight) * (1.0 - transmission)).clamp(0.0, 1.0);
    let clearcoat_w = clearcoat * 0.25;
    let transmission_w = transmission;
    let sum = diffuse_w + specular_w + clearcoat_w + transmission_w;

    if sum <= 1.0e-6 {
        return lambert::sample(material, normal, wo, input);
    }

    let pick = input.u3.clamp(0.0, 1.0) * sum;
    if pick < diffuse_w {
        let mut s = lambert::sample(material, normal, wo, input);
        s.f = eval(material, normal, s.wi, wo);
        s.pdf = pdf(material, normal, s.wi, wo).max(1.0e-6);
        return s;
    }

    if pick < diffuse_w + specular_w {
        let specular_params = MaterialParams::metal(
            blended_f0(material),
            material.roughness,
            Spectrum::black(),
            0.0,
        );
        let mut s = metal::sample(specular_params, normal, wo, input);
        s.f = eval(material, normal, s.wi, wo);
        s.pdf = pdf(material, normal, s.wi, wo).max(1.0e-6);
        return s;
    }

    if pick < diffuse_w + specular_w + clearcoat_w {
        let clearcoat_params = MaterialParams::metal(
            Spectrum::rgb(1.0, 1.0, 1.0),
            material.clearcoat_roughness.clamp(0.02, 1.0),
            Spectrum::black(),
            0.0,
        );
        let mut s = metal::sample(clearcoat_params, normal, wo, input);
        s.f = eval(material, normal, s.wi, wo);
        s.pdf = pdf(material, normal, s.wi, wo).max(1.0e-6);
        return s;
    }

    let dielectric_params = MaterialParams::dielectric(
        material.color,
        material.ior,
        material.roughness,
        material.thin_walled,
        Spectrum::black(),
        0.0,
    );
    let mut s = super::dielectric::sample(dielectric_params, normal, wo, input);
    s.f = if s.transmission {
        material.color
    } else {
        eval(material, normal, s.wi, wo)
    };
    s.pdf = if s.delta {
        s.pdf.max(1.0e-6)
    } else {
        pdf(material, normal, s.wi, wo).max(1.0e-6)
    };
    s
}

fn fresnel_f0_from_ior(ior: f32) -> f32 {
    let i = ior.max(1.0e-3);
    let r = (i - 1.0) / (i + 1.0);
    r * r
}

fn blended_f0(material: MaterialParams) -> Spectrum {
    let f0_dielectric = fresnel_f0_from_ior(material.ior).clamp(0.0, 1.0);
    let base_f0 = Spectrum::rgb(f0_dielectric, f0_dielectric, f0_dielectric)
        .scale(material.specular.clamp(0.0, 1.0) * material.specular_weight.clamp(0.0, 1.0))
        * material.specular_color;
    lerp_spectrum(base_f0, material.color, material.metallic.clamp(0.0, 1.0))
}

fn lerp_spectrum(a: Spectrum, b: Spectrum, t: f32) -> Spectrum {
    a.scale(1.0 - t) + b.scale(t)
}
