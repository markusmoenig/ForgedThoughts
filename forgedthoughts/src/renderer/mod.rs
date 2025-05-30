pub mod pbr;

use crate::prelude::*;
use std::sync::Arc;
use vek::{Vec2, Vec3, Vec4};

#[allow(unused)]
pub trait Renderer: Send + Sync {
    fn new() -> Self
    where
        Self: Sized;

    /// Returns the name of the renderer.
    fn name(&self) -> &str;

    /// Render the pixel at the given screen position.
    fn render(
        &self,
        uv: Vec2<F>,
        resolution: Vec2<F>,
        ft: Arc<FT>,
        model: Arc<ModelBuffer>,
    ) -> Vec4<F> {
        Vec4::zero()
    }

    /// Get the background color.
    fn background_color(&mut self) -> Vec3<F> {
        Vec3::zero()
    }

    /// Set the background color.
    fn set_background_color(&mut self, color: Vec3<F>) {}

    /// Converts an sRGB Vec3 to linear space.
    #[inline(always)]
    fn srgb_to_linear(&self, v: Vec3<F>) -> Vec3<F> {
        v.map(|c| {
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        })
    }

    /// Converts a linear Vec3 to sRGB space.
    #[inline(always)]
    fn linear_to_srgb(&self, v: Vec3<F>) -> Vec3<F> {
        v.map(|c| {
            if c <= 0.0031308 {
                c * 12.92
            } else {
                1.055 * c.powf(1.0 / 2.4) - 0.055
            }
        })
    }

    /// Reflects vector `v` about normal `n`.
    #[inline(always)]
    fn reflect(&self, v: Vec3<F>, n: Vec3<F>) -> Vec3<F> {
        v - 2.0 * v.dot(n) * n
    }

    /// Computes the refraction direction using Snell's law.
    /// Returns `None` if total internal reflection occurs.
    #[inline(always)]
    fn refract(&self, v: Vec3<F>, n: Vec3<F>, eta: F) -> Option<Vec3<F>> {
        let cos_i = (-v).dot(n);
        let sin2_t = eta * eta * (1.0 - cos_i * cos_i);

        if sin2_t > 1.0 {
            None // Total internal reflection
        } else {
            let cos_t = (1.0 - sin2_t).sqrt();
            Some(eta * v + (eta * cos_i - cos_t) * n)
        }
    }

    fn modulo(&self, uv: Vec2<F>) -> Vec3<F> {
        let scale = 10.0;
        let uv = uv * scale;
        let check = ((uv.x.floor() as i32 + uv.y.floor() as i32) & 1) as F;
        Vec3::broadcast(0.2 + check * 0.3)
    }
}
