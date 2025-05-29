pub mod pinhole;

use crate::prelude::*;
use vek::{Vec2, Vec3};

#[allow(unused)]
pub trait Camera: Send + Sync {
    fn new() -> Self
    where
        Self: Sized;

    /// Returns the name of the camera.
    fn name(&self) -> &str;

    /// Set the origin of the camera.
    fn set_origin(&mut self, origin: Vec3<F>) {}

    /// Set the center of the camera.
    fn set_center(&mut self, center: Vec3<F>) {}

    /// Set the fov of the camera.
    fn set_fov(&mut self, fov: F) {}

    /// Create a ray.
    fn create_ray(&self, uv: Vec2<F>, screen_size: Vec2<F>, offset: Vec2<F>) -> Ray;
}
