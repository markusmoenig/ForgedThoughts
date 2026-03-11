use std::f32::consts::PI;
use std::ops::{Add, Mul, Sub};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn dot(self, rhs: Self) -> f32 {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z
    }

    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    pub fn normalize(self) -> Self {
        let len = self.length();
        if len <= f32::EPSILON {
            self
        } else {
            self * (1.0 / len)
        }
    }

    pub fn cross(self, rhs: Self) -> Self {
        Self::new(
            self.y * rhs.z - self.z * rhs.y,
            self.z * rhs.x - self.x * rhs.z,
            self.x * rhs.y - self.y * rhs.x,
        )
    }
}

impl Add for Vec3 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl Sub for Vec3 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl Mul<f32> for Vec3 {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self::new(self.x * rhs, self.y * rhs, self.z * rhs)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spectrum {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl Spectrum {
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b }
    }

    pub const fn black() -> Self {
        Self::rgb(0.0, 0.0, 0.0)
    }

    pub fn scale(self, s: f32) -> Self {
        Self::rgb(self.r * s, self.g * s, self.b * s)
    }
}

impl Add for Spectrum {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::rgb(self.r + rhs.r, self.g + rhs.g, self.b + rhs.b)
    }
}

impl Mul for Spectrum {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self::rgb(self.r * rhs.r, self.g * rhs.g, self.b * rhs.b)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}

pub trait Camera: Send + Sync {
    fn generate_ray(&self, ndc_x: f32, ndc_y: f32) -> Ray;
}

#[derive(Debug, Clone, Copy)]
pub struct PinholeCamera {
    pub origin: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub fov_y_degrees: f32,
}

impl Default for PinholeCamera {
    fn default() -> Self {
        Self {
            origin: Vec3::new(0.0, 0.0, 6.0),
            target: Vec3::new(0.0, 0.0, 0.0),
            up: Vec3::new(0.0, 1.0, 0.0),
            fov_y_degrees: 45.0,
        }
    }
}

impl Camera for PinholeCamera {
    fn generate_ray(&self, ndc_x: f32, ndc_y: f32) -> Ray {
        let forward = (self.target - self.origin).normalize();
        let right = forward.cross(self.up).normalize();
        let up = right.cross(forward).normalize();
        let tan_fov = (0.5 * self.fov_y_degrees.to_radians()).tan();
        let dir = (forward + right * (ndc_x * tan_fov) + up * (ndc_y * tan_fov)).normalize();
        Ray {
            origin: self.origin,
            direction: dir,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightSample {
    pub wi: Vec3,
    pub radiance: Spectrum,
    pub distance: f32,
}

pub trait Light: Send + Sync {
    fn sample_li(&self, point: Vec3) -> LightSample;
    fn sample_li_indexed(
        &self,
        point: Vec3,
        _sample_index: u32,
        _sample_count: u32,
    ) -> LightSample {
        self.sample_li(point)
    }
    fn shadow_sample_count(&self) -> u32 {
        1
    }
    fn emitted_radiance(&self, _dir: Vec3) -> Spectrum {
        Spectrum::black()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PointLight {
    pub position: Vec3,
    pub intensity: Spectrum,
}

impl Light for PointLight {
    fn sample_li(&self, point: Vec3) -> LightSample {
        let to_light = self.position - point;
        let distance = to_light.length().max(1.0e-4);
        let attenuation = 1.0 / (distance * distance);
        LightSample {
            wi: to_light * (1.0 / distance),
            radiance: self.intensity.scale(attenuation),
            distance,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EnvLight {
    pub radiance: Spectrum,
}

impl Light for EnvLight {
    fn sample_li(&self, _point: Vec3) -> LightSample {
        LightSample {
            wi: Vec3::new(0.0, 1.0, 0.0),
            radiance: self.radiance,
            distance: f32::INFINITY,
        }
    }

    fn emitted_radiance(&self, _dir: Vec3) -> Spectrum {
        self.radiance
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SphereLight {
    pub position: Vec3,
    pub radius: f32,
    pub intensity: Spectrum,
    pub samples: u32,
}

impl Light for SphereLight {
    fn sample_li(&self, point: Vec3) -> LightSample {
        self.sample_li_indexed(point, 0, self.shadow_sample_count())
    }

    fn sample_li_indexed(&self, point: Vec3, sample_index: u32, sample_count: u32) -> LightSample {
        if self.radius <= 1.0e-5 {
            let to_light = self.position - point;
            let distance = to_light.length().max(1.0e-4);
            let attenuation = 1.0 / (distance * distance);
            return LightSample {
                wi: to_light * (1.0 / distance),
                radiance: self.intensity.scale(attenuation),
                distance,
            };
        }

        let normal = (point - self.position).normalize();
        let up = if normal.y.abs() < 0.99 {
            Vec3::new(0.0, 1.0, 0.0)
        } else {
            Vec3::new(1.0, 0.0, 0.0)
        };
        let tangent = normal.cross(up).normalize();
        let bitangent = tangent.cross(normal).normalize();
        let (u1, u2) = hammersley_2d(sample_index, sample_count.max(1));
        let disk = concentric_disk_sample(u1, u2);
        let sample_pos =
            self.position + tangent * (disk.x * self.radius) + bitangent * (disk.y * self.radius);
        let to_light = sample_pos - point;
        let distance = to_light.length().max(1.0e-4);
        let attenuation = 1.0 / (distance * distance);
        LightSample {
            wi: to_light * (1.0 / distance),
            radiance: self.intensity.scale(attenuation),
            distance,
        }
    }

    fn shadow_sample_count(&self) -> u32 {
        self.samples.max(1)
    }
}

fn hammersley_2d(i: u32, n: u32) -> (f32, f32) {
    let u = (i as f32 + 0.5) / n.max(1) as f32;
    let mut bits = i;
    bits = bits.rotate_right(16);
    bits = ((bits & 0x5555_5555) << 1) | ((bits & 0xAAAA_AAAA) >> 1);
    bits = ((bits & 0x3333_3333) << 2) | ((bits & 0xCCCC_CCCC) >> 2);
    bits = ((bits & 0x0F0F_0F0F) << 4) | ((bits & 0xF0F0_F0F0) >> 4);
    bits = ((bits & 0x00FF_00FF) << 8) | ((bits & 0xFF00_FF00) >> 8);
    let v = bits as f32 * 2.328_306_4e-10;
    (u, v)
}

fn concentric_disk_sample(u1: f32, u2: f32) -> Vec3 {
    let sx = 2.0 * u1 - 1.0;
    let sy = 2.0 * u2 - 1.0;
    if sx.abs() <= f32::EPSILON && sy.abs() <= f32::EPSILON {
        return Vec3::new(0.0, 0.0, 0.0);
    }

    let (r, theta) = if sx.abs() > sy.abs() {
        (sx, std::f32::consts::FRAC_PI_4 * (sy / sx))
    } else {
        (
            sy,
            std::f32::consts::FRAC_PI_2 - std::f32::consts::FRAC_PI_4 * (sx / sy),
        )
    };
    Vec3::new(r * theta.cos(), r * theta.sin(), 0.0)
}

#[derive(Debug, Clone, Copy)]
pub struct SurfaceHit {
    pub position: Vec3,
    pub normal: Vec3,
}

pub trait Bsdf: Send + Sync {
    fn evaluate(&self, normal: Vec3, wi: Vec3, wo: Vec3) -> Spectrum;
}

pub trait MaterialModel: Send + Sync {
    fn name(&self) -> &'static str;
    fn make_bsdf(&self, hit: SurfaceHit) -> Box<dyn Bsdf>;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OpenPbrMaterial {
    pub base_color: Spectrum,
    pub roughness: f32,
    pub metallic: f32,
    pub specular: f32,
    pub specular_weight: f32,
    pub specular_color: Spectrum,
    pub ior: f32,
    pub clearcoat: f32,
    pub clearcoat_roughness: f32,
    pub transmission: f32,
    pub thin_walled: f32,
    pub anisotropy: f32,
    pub anisotropy_rotation: f32,
    pub emission_color: Spectrum,
    pub emission_strength: f32,
}

impl Default for OpenPbrMaterial {
    fn default() -> Self {
        Self {
            base_color: Spectrum::rgb(0.8, 0.8, 0.8),
            roughness: 0.5,
            metallic: 0.0,
            specular: 0.5,
            specular_weight: 1.0,
            specular_color: Spectrum::rgb(1.0, 1.0, 1.0),
            ior: 1.5,
            clearcoat: 0.0,
            clearcoat_roughness: 0.1,
            transmission: 0.0,
            thin_walled: 0.0,
            anisotropy: 0.0,
            anisotropy_rotation: 0.0,
            emission_color: Spectrum::black(),
            emission_strength: 0.0,
        }
    }
}

impl MaterialModel for OpenPbrMaterial {
    fn name(&self) -> &'static str {
        "openpbr"
    }

    fn make_bsdf(&self, _hit: SurfaceHit) -> Box<dyn Bsdf> {
        Box::new(OpenPbrBsdf { material: *self })
    }
}

#[derive(Debug, Clone, Copy)]
struct OpenPbrBsdf {
    material: OpenPbrMaterial,
}

impl Bsdf for OpenPbrBsdf {
    fn evaluate(&self, normal: Vec3, wi: Vec3, wo: Vec3) -> Spectrum {
        let ndotl = normal.dot(wi).max(0.0);
        let ndotv = normal.dot(wo).max(0.0);
        if ndotl <= 0.0 || ndotv <= 0.0 {
            return Spectrum::black();
        }

        let roughness = self.material.roughness.clamp(0.02, 1.0);
        let alpha = roughness * roughness;
        let h = (wi + wo).normalize();
        let ndoth = normal.dot(h).max(0.0);
        let vdoth = wo.dot(h).max(0.0);

        let f0_dielectric = fresnel_f0_from_ior(self.material.ior).clamp(0.0, 1.0);
        let base_f0 = Spectrum::rgb(f0_dielectric, f0_dielectric, f0_dielectric).scale(
            self.material.specular.clamp(0.0, 1.0) * self.material.specular_weight.clamp(0.0, 1.0),
        ) * self.material.specular_color;
        let metal_f0 = self.material.base_color;
        let f0 = lerp_spectrum(base_f0, metal_f0, self.material.metallic.clamp(0.0, 1.0));
        let fresnel = fresnel_schlick(f0, vdoth);
        let d = ggx_d(ndoth, alpha);
        let g = smith_ggx_g(ndotl, ndotv, alpha);
        let denom = (4.0 * ndotl * ndotv).max(1.0e-5);
        let specular = fresnel.scale((d * g) / denom);

        let kd = 1.0 - self.material.metallic.clamp(0.0, 1.0);
        let diffuse = self.material.base_color.scale((kd / PI) * ndotl);

        let clearcoat = self.material.clearcoat.clamp(0.0, 1.0);
        let clearcoat_alpha = self.material.clearcoat_roughness.clamp(0.02, 1.0).powi(2);
        let clearcoat_d = ggx_d(ndoth, clearcoat_alpha);
        let clearcoat_g = smith_ggx_g(ndotl, ndotv, clearcoat_alpha);
        let clearcoat_f = fresnel_schlick_scalar(0.04, vdoth);
        let clearcoat_spec = clearcoat * ((clearcoat_d * clearcoat_g * clearcoat_f) / denom);
        let clearcoat_term = Spectrum::rgb(clearcoat_spec, clearcoat_spec, clearcoat_spec);

        diffuse + specular.scale(ndotl) + clearcoat_term.scale(ndotl)
    }
}

fn fresnel_f0_from_ior(ior: f32) -> f32 {
    let i = ior.max(1.0e-3);
    let r = (i - 1.0) / (i + 1.0);
    r * r
}

fn fresnel_schlick(f0: Spectrum, cos_theta: f32) -> Spectrum {
    let m = (1.0 - cos_theta.clamp(0.0, 1.0)).powi(5);
    f0 + Spectrum::rgb(1.0, 1.0, 1.0).scale(m) + f0.scale(-m)
}

fn fresnel_schlick_scalar(f0: f32, cos_theta: f32) -> f32 {
    let m = (1.0 - cos_theta.clamp(0.0, 1.0)).powi(5);
    f0 + (1.0 - f0) * m
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
    let a = alpha;
    let k = (a * a) * 0.5;
    ndotx / (ndotx * (1.0 - k) + k).max(1.0e-5)
}

fn lerp_spectrum(a: Spectrum, b: Spectrum, t: f32) -> Spectrum {
    let tt = t.clamp(0.0, 1.0);
    Spectrum::rgb(
        a.r * (1.0 - tt) + b.r * tt,
        a.g * (1.0 - tt) + b.g * tt,
        a.b * (1.0 - tt) + b.b * tt,
    )
}

pub trait Integrator: Send + Sync {
    fn render_pixel(
        &self,
        camera: &dyn Camera,
        lights: &[Box<dyn Light>],
        material: &dyn MaterialModel,
        ndc_x: f32,
        ndc_y: f32,
    ) -> Spectrum;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct PreviewIntegrator;

impl Integrator for PreviewIntegrator {
    fn render_pixel(
        &self,
        camera: &dyn Camera,
        lights: &[Box<dyn Light>],
        material: &dyn MaterialModel,
        ndc_x: f32,
        ndc_y: f32,
    ) -> Spectrum {
        let ray = camera.generate_ray(ndc_x, ndc_y);
        let hit = SurfaceHit {
            position: Vec3::new(0.0, 0.0, 0.0),
            normal: Vec3::new(0.0, 0.0, 1.0),
        };
        let bsdf = material.make_bsdf(hit);
        let mut out = Spectrum::black();
        for light in lights {
            let li = light.sample_li(hit.position);
            let f = bsdf.evaluate(hit.normal, li.wi, ray.direction * -1.0);
            out = out + (f * li.radiance);
        }
        out
    }
}

pub enum CameraKind {
    Pinhole(PinholeCamera),
}

impl Camera for CameraKind {
    fn generate_ray(&self, ndc_x: f32, ndc_y: f32) -> Ray {
        match self {
            Self::Pinhole(camera) => camera.generate_ray(ndc_x, ndc_y),
        }
    }
}

pub enum MaterialKind {
    OpenPbr(OpenPbrMaterial),
}

impl MaterialModel for MaterialKind {
    fn name(&self) -> &'static str {
        match self {
            Self::OpenPbr(mat) => mat.name(),
        }
    }

    fn make_bsdf(&self, hit: SurfaceHit) -> Box<dyn Bsdf> {
        match self {
            Self::OpenPbr(mat) => mat.make_bsdf(hit),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Camera, EnvLight, Integrator, MaterialModel, OpenPbrMaterial, PinholeCamera, PointLight,
        PreviewIntegrator, Spectrum, Vec3,
    };

    #[test]
    fn pinhole_camera_generates_normalized_rays() {
        let camera = PinholeCamera::default();
        let ray = camera.generate_ray(0.0, 0.0);
        let len = ray.direction.length();
        assert!((len - 1.0).abs() < 1.0e-5);
    }

    #[test]
    fn openpbr_material_exposes_model_name() {
        let mat = OpenPbrMaterial::default();
        assert_eq!(mat.name(), "openpbr");
    }

    #[test]
    fn preview_integrator_produces_non_black_with_lights() {
        let camera = PinholeCamera::default();
        let material = OpenPbrMaterial::default();
        let lights: Vec<Box<dyn super::Light>> = vec![
            Box::new(PointLight {
                position: Vec3::new(2.0, 2.0, 2.0),
                intensity: Spectrum::rgb(8.0, 8.0, 8.0),
            }),
            Box::new(EnvLight {
                radiance: Spectrum::rgb(0.1, 0.1, 0.1),
            }),
        ];

        let integrator = PreviewIntegrator;
        let c = integrator.render_pixel(&camera, &lights, &material, 0.0, 0.0);
        assert!(c.r > 0.0 || c.g > 0.0 || c.b > 0.0);
    }
}
