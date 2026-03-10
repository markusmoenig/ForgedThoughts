mod dielectric;
mod lambert;
mod metal;

use crate::render_api::{Spectrum, Vec3};

pub type LambertMaterial = MaterialParams;
pub type MetalMaterial = MaterialParams;
pub type DielectricMaterial = MaterialParams;

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
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Material {
    Lambert(MaterialParams),
    Metal(MaterialParams),
    Dielectric(MaterialParams),
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
    pub fn emission(self) -> Spectrum {
        match self {
            Material::Lambert(m) | Material::Metal(m) | Material::Dielectric(m) => {
                m.emission_color.scale(m.emission_strength.max(0.0))
            }
        }
    }

    pub fn eval(self, normal: Vec3, wi: Vec3, wo: Vec3) -> Spectrum {
        match self {
            Material::Lambert(m) => lambert::eval(m, normal, wi, wo),
            Material::Metal(m) => metal::eval(m, normal, wi, wo),
            Material::Dielectric(m) => dielectric::eval(m, normal, wi, wo),
        }
    }

    pub fn pdf(self, normal: Vec3, wi: Vec3, wo: Vec3) -> f32 {
        match self {
            Material::Lambert(m) => lambert::pdf(m, normal, wi, wo),
            Material::Metal(m) => metal::pdf(m, normal, wi, wo),
            Material::Dielectric(m) => dielectric::pdf(m, normal, wi, wo),
        }
    }

    pub fn sample(self, normal: Vec3, wo: Vec3, input: SampleInput) -> BsdfSample {
        match self {
            Material::Lambert(m) => lambert::sample(m, normal, wo, input),
            Material::Metal(m) => metal::sample(m, normal, wo, input),
            Material::Dielectric(m) => dielectric::sample(m, normal, wo, input),
        }
    }
}
