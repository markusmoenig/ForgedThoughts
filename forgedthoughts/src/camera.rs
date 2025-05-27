use crate::{Ray, F};
use vek::{Vec2, Vec3};

/// SDF
#[derive(PartialEq, Debug, Clone)]
pub struct Camera {
    pub origin: Vec3<F>,
    pub center: Vec3<F>,
    pub fov: F,
}

impl Default for Camera {
    fn default() -> Self {
        Self::new()
    }
}

impl Camera {
    pub fn new() -> Self {
        Self {
            origin: Vec3::new(0.0, 1.0, 3.0),
            center: Vec3::zero(),
            fov: 70.0,
        }
    }

    /// Create a camera ray. Have todo modular later on to support iso cameras etc.
    pub fn create_ray(&self, uv: Vec2<F>, cam_offset: Vec2<F>, screen_size: Vec2<F>) -> Ray {
        /*
        let ww = (center - origin).normalize();
        let uu = ww.cross(&F3::new(0.0, 1.0, 0.0)).normalize();
        let vv = uu.cross(&ww).normalize();

        let d = (uu.mult_f(&(uv.x * cam_offset.x)) + vv.mult_f(&(uv.y * cam_offset.y)) + ww.mult_f(&2.0)).normalize();

        [origin, d]
        */

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
        rd += horizontal * (pixel_size.x * cam_offset.x + uv.x);
        rd += vertical * (pixel_size.y * cam_offset.y + uv.y);

        Ray::new(origin, rd.normalized())
    }
}
