mod dielectric;
mod lambert;
mod metal;

use crate::render_api::{Spectrum, Vec3};

pub type LambertMaterial = MaterialParams;
pub type MetalMaterial = MaterialParams;
pub type DielectricMaterial = MaterialParams;

#[derive(Clone, Copy, PartialEq)]
pub enum MaterialKindTag {
    Lambert,
    Metal,
    Dielectric,
}

#[derive(Clone, Copy, PartialEq)]
pub struct MediumParams {
    pub ior: f32,
    pub absorption_color: Spectrum,
    pub density: f32,
}

impl MediumParams {
    #[must_use]
    pub fn new(ior: f32, absorption_color: Spectrum, density: f32) -> Self {
        Self {
            ior,
            absorption_color,
            density,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub struct SubsurfaceParams {
    pub color: Spectrum,
    pub radius: Vec3,
    pub anisotropy: f32,
    pub scale: f32,
}

impl SubsurfaceParams {
    #[must_use]
    pub fn new(color: Spectrum, radius: Vec3, anisotropy: f32, scale: f32) -> Self {
        Self {
            color,
            radius,
            anisotropy,
            scale,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum ColorPattern {
    Checker3d {
        color_a: Spectrum,
        color_b: Spectrum,
        scale: f32,
    },
}

#[derive(Clone, Copy, PartialEq)]
pub struct MaterialParams {
    pub color: Spectrum,
    pub roughness: f32,
    pub ior: f32,
    pub thin_walled: bool,
    pub emission_color: Spectrum,
    pub emission_strength: f32,
    pub medium: Option<MediumParams>,
    pub subsurface: Option<SubsurfaceParams>,
    pub pattern: Option<ColorPattern>,
    pub dynamic_material_id: Option<u32>,
    pub dynamic_override_id: Option<u32>,
}

impl MaterialParams {
    #[must_use]
    pub fn lambert(color: Spectrum, emission_color: Spectrum, emission_strength: f32) -> Self {
        Self {
            color,
            roughness: 1.0,
            ior: 1.5,
            thin_walled: false,
            emission_color,
            emission_strength,
            medium: None,
            subsurface: None,
            pattern: None,
            dynamic_material_id: None,
            dynamic_override_id: None,
        }
    }

    #[must_use]
    pub fn metal(
        color: Spectrum,
        roughness: f32,
        emission_color: Spectrum,
        emission_strength: f32,
    ) -> Self {
        Self {
            color,
            roughness,
            ior: 1.5,
            thin_walled: false,
            emission_color,
            emission_strength,
            medium: None,
            subsurface: None,
            pattern: None,
            dynamic_material_id: None,
            dynamic_override_id: None,
        }
    }

    #[must_use]
    pub fn dielectric(
        color: Spectrum,
        ior: f32,
        roughness: f32,
        thin_walled: bool,
        emission_color: Spectrum,
        emission_strength: f32,
    ) -> Self {
        Self {
            color,
            roughness,
            ior,
            thin_walled,
            emission_color,
            emission_strength,
            medium: None,
            subsurface: None,
            pattern: None,
            dynamic_material_id: None,
            dynamic_override_id: None,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Material {
    Lambert(MaterialParams),
    Metal(MaterialParams),
    Dielectric(MaterialParams),
    Blend(BlendedMaterial),
}

#[derive(Clone, Copy, PartialEq)]
pub struct BlendedMaterial {
    pub a_model: MaterialKindTag,
    pub a_params: MaterialParams,
    pub b_model: MaterialKindTag,
    pub b_params: MaterialParams,
    pub t: f32,
}

#[derive(Clone, Copy)]
pub struct BsdfSample {
    pub wi: Vec3,
    pub f: Spectrum,
    pub pdf: f32,
    pub delta: bool,
    pub apply_cos: bool,
    pub transmission: bool,
    pub thin_walled: bool,
    pub next_ior: f32,
}

#[derive(Clone, Copy)]
pub struct SampleInput {
    pub front_face: bool,
    pub current_ior: f32,
    pub u1: f32,
    pub u2: f32,
    pub u3: f32,
}

pub trait MaterialBsdf {
    fn emission(&self) -> Spectrum;
    fn eval(&self, normal: Vec3, wi: Vec3, wo: Vec3) -> Spectrum;
    fn pdf(&self, normal: Vec3, wi: Vec3, wo: Vec3) -> f32;
    fn sample(&self, normal: Vec3, wo: Vec3, input: SampleInput) -> BsdfSample;
}

impl Material {
    #[must_use]
    pub fn model(self) -> MaterialKindTag {
        match self {
            Material::Lambert(_) => MaterialKindTag::Lambert,
            Material::Metal(_) => MaterialKindTag::Metal,
            Material::Dielectric(_) => MaterialKindTag::Dielectric,
            Material::Blend(_) => MaterialKindTag::Lambert,
        }
    }

    #[must_use]
    pub fn params(self) -> MaterialParams {
        match self {
            Material::Lambert(m) | Material::Metal(m) | Material::Dielectric(m) => m,
            Material::Blend(m) => {
                if m.t < 0.5 {
                    m.a_params
                } else {
                    m.b_params
                }
            }
        }
    }

    pub fn emission(self) -> Spectrum {
        match self {
            Material::Lambert(m) | Material::Metal(m) | Material::Dielectric(m) => {
                m.emission_color.scale(m.emission_strength.max(0.0))
            }
            Material::Blend(m) => lerp_spectrum(
                material_emission(m.a_model, m.a_params),
                material_emission(m.b_model, m.b_params),
                m.t,
            ),
        }
    }

    pub fn eval(self, normal: Vec3, wi: Vec3, wo: Vec3) -> Spectrum {
        match self {
            Material::Lambert(m) => lambert::eval(m, normal, wi, wo),
            Material::Metal(m) => metal::eval(m, normal, wi, wo),
            Material::Dielectric(m) => dielectric::eval(m, normal, wi, wo),
            Material::Blend(m) => lerp_spectrum(
                material_eval(m.a_model, m.a_params, normal, wi, wo),
                material_eval(m.b_model, m.b_params, normal, wi, wo),
                m.t,
            ),
        }
    }

    pub fn pdf(self, normal: Vec3, wi: Vec3, wo: Vec3) -> f32 {
        match self {
            Material::Lambert(m) => lambert::pdf(m, normal, wi, wo),
            Material::Metal(m) => metal::pdf(m, normal, wi, wo),
            Material::Dielectric(m) => dielectric::pdf(m, normal, wi, wo),
            Material::Blend(m) => {
                let t = m.t.clamp(0.0, 1.0);
                material_pdf(m.a_model, m.a_params, normal, wi, wo) * (1.0 - t)
                    + material_pdf(m.b_model, m.b_params, normal, wi, wo) * t
            }
        }
    }

    pub fn sample(self, normal: Vec3, wo: Vec3, input: SampleInput) -> BsdfSample {
        match self {
            Material::Lambert(m) => lambert::sample(m, normal, wo, input),
            Material::Metal(m) => metal::sample(m, normal, wo, input),
            Material::Dielectric(m) => dielectric::sample(m, normal, wo, input),
            Material::Blend(m) => sample_blend(m, normal, wo, input),
        }
    }
}

fn material_emission(model: MaterialKindTag, params: MaterialParams) -> Spectrum {
    match model {
        MaterialKindTag::Lambert | MaterialKindTag::Metal | MaterialKindTag::Dielectric => params
            .emission_color
            .scale(params.emission_strength.max(0.0)),
    }
}

fn material_eval(
    model: MaterialKindTag,
    params: MaterialParams,
    normal: Vec3,
    wi: Vec3,
    wo: Vec3,
) -> Spectrum {
    match model {
        MaterialKindTag::Lambert => lambert::eval(params, normal, wi, wo),
        MaterialKindTag::Metal => metal::eval(params, normal, wi, wo),
        MaterialKindTag::Dielectric => dielectric::eval(params, normal, wi, wo),
    }
}

fn material_pdf(
    model: MaterialKindTag,
    params: MaterialParams,
    normal: Vec3,
    wi: Vec3,
    wo: Vec3,
) -> f32 {
    match model {
        MaterialKindTag::Lambert => lambert::pdf(params, normal, wi, wo),
        MaterialKindTag::Metal => metal::pdf(params, normal, wi, wo),
        MaterialKindTag::Dielectric => dielectric::pdf(params, normal, wi, wo),
    }
}

fn material_sample(
    model: MaterialKindTag,
    params: MaterialParams,
    normal: Vec3,
    wo: Vec3,
    input: SampleInput,
) -> BsdfSample {
    match model {
        MaterialKindTag::Lambert => lambert::sample(params, normal, wo, input),
        MaterialKindTag::Metal => metal::sample(params, normal, wo, input),
        MaterialKindTag::Dielectric => dielectric::sample(params, normal, wo, input),
    }
}

fn sample_blend(
    material: BlendedMaterial,
    normal: Vec3,
    wo: Vec3,
    input: SampleInput,
) -> BsdfSample {
    let t = material.t.clamp(0.0, 1.0);
    let choose_b = input.u3 < t;
    let chosen = if choose_b {
        material_sample(material.b_model, material.b_params, normal, wo, input)
    } else {
        material_sample(material.a_model, material.a_params, normal, wo, input)
    };
    let f = lerp_spectrum(
        material_eval(material.a_model, material.a_params, normal, chosen.wi, wo),
        material_eval(material.b_model, material.b_params, normal, chosen.wi, wo),
        t,
    );
    let pdf = (material_pdf(material.a_model, material.a_params, normal, chosen.wi, wo)
        * (1.0 - t)
        + material_pdf(material.b_model, material.b_params, normal, chosen.wi, wo) * t)
        .max(1.0e-6);
    let all_delta = matches!(material.a_model, MaterialKindTag::Dielectric)
        && matches!(material.b_model, MaterialKindTag::Dielectric);
    BsdfSample {
        wi: chosen.wi,
        f,
        pdf,
        delta: chosen.delta && all_delta,
        apply_cos: chosen.apply_cos,
        transmission: chosen.transmission,
        thin_walled: chosen.thin_walled,
        next_ior: chosen.next_ior,
    }
}

fn lerp_spectrum(a: Spectrum, b: Spectrum, t: f32) -> Spectrum {
    let tt = t.clamp(0.0, 1.0);
    Spectrum::rgb(
        a.r * (1.0 - tt) + b.r * tt,
        a.g * (1.0 - tt) + b.g * tt,
        a.b * (1.0 - tt) + b.b * tt,
    )
}
