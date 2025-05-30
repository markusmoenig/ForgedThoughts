pub mod point;

use crate::prelude::*;
use vek::Vec3;

#[allow(unused)]
pub trait Light: Send + Sync {
    fn new() -> Self
    where
        Self: Sized;

    /// Returns the name of the camera.
    fn name(&self) -> &str;

    /// Get the position of the light.
    fn position(&self) -> Vec3<F> {
        Vec3::zero()
    }

    /// Set the origin of the light.
    fn set_position(&mut self, origin: Vec3<F>) {}

    /// Get the position of the light.
    fn color(&self) -> Vec3<F> {
        Vec3::one()
    }

    /// Set the color of the light.
    fn set_color(&mut self, color: Vec3<F>) {}

    /// Get the intensity of the light.
    fn intensity(&self) -> F {
        0.0
    }

    /// Set the intensity of the light.
    fn set_intensity(&mut self, fov: F) {}

    /// Get the radius of the light.
    fn radius(&self) -> F {
        0.0
    }

    /// Set the radius of the light for pointlights.
    fn set_radius(&mut self, fov: F) {}
}
