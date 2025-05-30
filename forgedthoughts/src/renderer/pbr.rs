use crate::prelude::*;
use crate::F_PI;
use std::sync::Arc;
use vek::{Vec2, Vec3, Vec4};

use rand::Rng;

#[allow(dead_code)]
#[allow(clippy::upper_case_acronyms)]
enum ReflectionType {
    Diffuse,
    Specular,
    Refractive,
    GGX,
}

pub struct PBR {
    pub background_color: Vec3<F>,
}

impl Renderer for PBR {
    fn new() -> Self
    where
        Self: Sized,
    {
        Self {
            background_color: Vec3::broadcast(0.2),
        }
    }

    fn name(&self) -> &str {
        "PBR"
    }

    /// Get the background color.
    fn background_color(&mut self) -> Vec3<F> {
        self.background_color
    }

    /// Set the background color.
    fn set_background_color(&mut self, color: Vec3<F>) {
        self.background_color = color;
    }

    /// Render the pixel at the given screen position.
    fn render(
        &self,
        uv: Vec2<F>,
        resolution: Vec2<F>,
        ft: Arc<FT>,
        model: Arc<ModelBuffer>,
    ) -> Vec4<F> {
        let mut rng = rand::rng();

        let mut acc = Vec3::<F>::zero();
        let mut mask = Vec3::<F>::one();

        let eps = model.voxel_size_min();

        let mut ray =
            ft.graph
                .camera
                .create_ray(uv, resolution, Vec2::new(rng.random(), rng.random()));

        for _ in 0..4 {
            let Some(hit) = model.raymarch(&ray) else {
                acc += self.srgb_to_linear(self.background_color) * mask;
                // acc += self.srgb_to_linear(self.modulo(uv)) * mask;
                break;
            };

            let material = ft.graph.evaluate_material(hit.voxel.material as usize, hit);
            let albedo = material.albedo_linear();

            let x = hit.position;
            let n = hit.normal;
            let nl = if n.dot(ray.dir) < 0.0 { n } else { -n };

            let reflection_type = ReflectionType::GGX;

            #[allow(clippy::single_match)]
            match reflection_type {
                ReflectionType::Diffuse => {
                    let r2: F = rng.random();
                    let phi = 2.0 * std::f32::consts::PI * rng.random::<F>();
                    let d = Self::jitter(nl, phi, r2.sqrt(), (1.0 - r2).sqrt());

                    // Direct lighting sample
                    let mut e = Vec3::zero();

                    for light in &ft.graph.lights {
                        let l0 = light.position() - x;
                        let cos_a_max = ((1.0 - (light.radius() * light.radius() / l0.dot(l0)))
                            .max(0.0))
                        .sqrt();
                        let cosa = lerp(cos_a_max, 1.0, rng.random());
                        let l = Self::jitter(
                            l0,
                            2.0 * std::f32::consts::PI * rng.random::<F>(),
                            (1.0 - cosa * cosa).sqrt(),
                            cosa,
                        );

                        let shadow_origin = x + nl * 1e-3;
                        let light_ray = Ray::new(shadow_origin, l);
                        if light_ray
                            .intersect_sphere(light.position(), light.radius())
                            .is_some()
                        {
                            let omega = 2.0 * std::f32::consts::PI * (1.0 - cos_a_max);
                            e += light.color() * n.dot(l).max(0.0) * omega / F_PI;
                        }
                    }

                    acc += mask * material.emission + mask * albedo * e;
                    mask *= albedo;
                    ray = Ray::new(x, d).advanced(eps);
                }
                ReflectionType::Specular => {
                    acc += mask * material.emission;
                    mask *= material.albedo;
                    ray = Ray::new(x, ray.dir - 2.0 * ray.dir.dot(n) * n).advanced(eps);
                }
                ReflectionType::Refractive => {
                    let a = n.dot(ray.dir);
                    let ddn = a.abs();
                    let (nc, nt) = (1.0, material.ior);
                    let into = a < 0.0;
                    let nnt = if into { nc / nt } else { nt / nc };
                    let cos2t = 1.0 - nnt * nnt * (1.0 - ddn * ddn);

                    ray = Ray::new(x, ray.dir - 2.0 * ray.dir.dot(n) * n).advanced(eps);

                    if cos2t > 0.0 {
                        let tdir = (ray.dir * nnt
                            + n * ((if into { 1.0 } else { -1.0 }) * (ddn * nnt + cos2t.sqrt())))
                        .normalized();
                        let r0 = ((nt - nc) / (nt + nc)).powi(2);
                        let c = 1.0 - if into { -ray.dir.dot(n) } else { tdir.dot(n) };
                        let re = r0 + (1.0 - r0) * c.powi(5);
                        let p = 0.25 + 0.5 * re;
                        let rp = re / p;
                        let tp = (1.0 - re) / (1.0 - p);

                        if rng.random::<F>() < p {
                            mask *= rp;
                        } else {
                            mask *= albedo * tp;
                            ray = Ray::new(x, tdir).advanced(eps);
                        }
                    }
                }
                ReflectionType::GGX => {
                    let roughness = material.roughness.clamp(0.001, 1.0);
                    let alpha = roughness * roughness;
                    let metallic = material.metallic;
                    let reflectance = 0.5;
                    let base_f0 = 0.04;
                    let f0 = base_f0 + (1.0 - base_f0) * metallic; // 0.04 for dielectrics → 1.0 for metal
                    let v = (-ray.dir).normalized();
                    let nv = n.dot(v).max(1e-5);
                    let color = albedo;

                    if rng.random::<F>() < reflectance {
                        // GGX
                        let mut brdf = Vec3::zero();

                        for light in &ft.graph.lights {
                            let l0 = light.position() - x;
                            let cos_a_max =
                                ((1.0 - (light.radius() * light.radius() / l0.dot(l0))).max(0.0))
                                    .sqrt();
                            let cosa = lerp(cos_a_max, 1.0, rng.random::<F>());

                            let l = Self::jitter(
                                l0,
                                2.0 * F_PI * rng.random::<F>(),
                                (1.0 - cosa * cosa).sqrt(),
                                cosa,
                            )
                            .normalized();

                            let shadow_origin = x + nl * eps;
                            if Ray::new(shadow_origin, l)
                                .intersect_sphere(light.position(), light.radius())
                                .is_some()
                            {
                                let omega = 2.0 * F_PI * (1.0 - cos_a_max);
                                let g = Self::ggx(nl, v, l, roughness, f0).clamp(0.0, 1.0);
                                brdf += light.color() * g * omega / nv; // F_PI;
                            }
                        }

                        let xsi_1: F = rng.random();
                        let xsi_2: F = rng.random();
                        let phi = ((alpha * xsi_1.sqrt()) / (1.0 - xsi_1).sqrt()).atan();
                        let theta = 2.0 * F_PI * xsi_2;
                        let dir = Self::angle_to_dir(nl, theta, phi);

                        acc += mask * material.emission + mask * color * brdf;
                        mask *= color;
                        ray = Ray::new(x, dir).advanced(eps);
                    } else {
                        // ── diffuse fallback
                        let r2: F = rng.random();
                        let d = Self::jitter(
                            nl,
                            2.0 * F_PI * rng.random::<F>(),
                            r2.sqrt(),
                            (1.0 - r2).sqrt(),
                        );

                        let mut e = Vec3::zero();
                        for light in &ft.graph.lights {
                            let l0 = light.position() - x;
                            let cos_a_max =
                                ((1.0 - (light.radius() * light.radius() / l0.dot(l0))).max(0.0))
                                    .sqrt();
                            let cosa = lerp(cos_a_max, 1.0, rng.random::<F>());
                            let l = Self::jitter(
                                l0,
                                2.0 * F_PI * rng.random::<F>(),
                                (1.0 - cosa * cosa).sqrt(),
                                cosa,
                            )
                            .normalized();

                            let shadow_origin = x + nl * eps;
                            if Ray::new(shadow_origin, l)
                                .intersect_sphere(light.position(), light.radius())
                                .is_some()
                            {
                                let omega = 2.0 * F_PI * (1.0 - cos_a_max);
                                e += light.color() * l.dot(n).max(0.0) * omega / F_PI;
                            }
                        }

                        acc += mask * material.emission + mask * color * e;
                        mask *= color;
                        ray = Ray::new(x, d).advanced(eps);
                    }
                }
            }
        }

        Vec4::new(acc.x, acc.y, acc.z, 1.0)
    }
}

pub trait PBRTrait {
    fn jitter(d: Vec3<F>, phi: F, sina: F, cosa: F) -> Vec3<F>;
    fn ggx(n: Vec3<F>, v: Vec3<F>, l: Vec3<F>, roughness: F, f0: F) -> F;
    fn angle_to_dir(n: Vec3<F>, theta: F, phi: F) -> Vec3<F>;
}

impl PBRTrait for PBR {
    /// Jitters a direction vector `d` by a cone angle (defined by `sina` and `cosa`)
    /// and a random azimuthal angle `phi`.
    fn jitter(d: Vec3<F>, phi: F, sina: F, cosa: F) -> Vec3<F> {
        let w = d.normalized();
        let u = Vec3::new(w.y, w.z, w.x).cross(w).normalized();
        let v = w.cross(u);
        (u * phi.cos() + v * phi.sin()) * sina + w * cosa
    }

    fn ggx(n: Vec3<F>, v: Vec3<F>, l: Vec3<F>, roughness: F, f0: F) -> F {
        let h = (v + l).normalized();
        let nl = n.dot(l).max(0.0);
        let nv = n.dot(v).max(0.0);
        let nh = n.dot(h).max(0.0);
        let lh = l.dot(h).max(0.0);

        let alpha = roughness * roughness + 1e-4;
        let a2 = alpha * alpha;

        // Normal-distribution (GGX)
        let denom = nh * nh * (a2 - 1.0) + 1.0;
        let d = a2 / (denom * denom);

        // Fresnel (Schlick)
        let fresnel_weight = (1.0 - lh).powf(5.0);
        let f = f0 + (1.0 - f0) * fresnel_weight;

        // Smith shadow-mask (height-correlated form)
        let k = (alpha + 1.0).powi(2) / 8.0;
        let g1_l = nl / (nl * (1.0 - k) + k);
        let g1_v = nv / (nv * (1.0 - k) + k);
        let g = g1_l * g1_v;

        (d * f * g * 0.25).max(0.0)
    }

    fn angle_to_dir(n: Vec3<F>, theta: F, phi: F) -> Vec3<F> {
        let sin_phi = phi.sin();
        let cos_phi = phi.cos();
        let w = n.normalized();
        let u = Vec3::new(w.y, w.z, w.x).cross(w).normalized();
        let v = w.cross(u);
        (u * theta.cos() + v * theta.sin()) * sin_phi + w * cos_phi
    }
}
