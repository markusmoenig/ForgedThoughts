use crate::prelude::*;
use vek::{Vec2, Vec3};

use crate::camera::Camera;

pub struct Pinhole {
    pub origin: Vec3<F>,
    pub center: Vec3<F>,
    pub fov: F,
}

impl Camera for Pinhole {
    fn new() -> Self
    where
        Self: Sized,
    {
        Self {
            origin: Vec3::new(0.0, 1.0, 3.0),
            center: Vec3::zero(),
            fov: 70.0,
        }
    }

    fn name(&self) -> &str {
        "Pinhole"
    }

    fn set_origin(&mut self, origin: Vec3<F>) {
        self.origin = origin;
    }

    fn set_center(&mut self, center: Vec3<F>) {
        self.center = center;
    }

    fn set_fov(&mut self, fov: F) {
        self.fov = fov;
    }

    /// Create a camera ray.
    fn create_ray(&self, uv: Vec2<F>, screen_size: Vec2<F>, offset: Vec2<F>) -> Ray {
        let origin = self.origin;
        let center = self.center;
        let fov = self.fov;

        let ratio = screen_size.x / screen_size.y;

        let pixel_size = Vec2::new(1.0 / screen_size.x, 1.0 / screen_size.y);

        let t = (fov.to_radians() * 0.5).tan();

        let half_width = Vec3::new(t, t, t);
        let half_height = half_width / ratio;

        let up_vector = Vec3::new(0.0, 1.0, 0.0);

        let w = (origin - center).normalized();
        let u = Vec3::cross(up_vector, w);
        let v = Vec3::cross(w, u);

        let lower_left = origin - half_width * u - half_height * v - w;
        let horizontal = u * half_width * 2.0;
        let vertical = v * half_height * 2.0;

        let mut rd = lower_left - origin;
        rd += horizontal * (pixel_size.x * offset.x + uv.x);
        rd += vertical * (pixel_size.y * offset.y + uv.y);

        Ray::new(origin, rd.normalized())
    }
}
