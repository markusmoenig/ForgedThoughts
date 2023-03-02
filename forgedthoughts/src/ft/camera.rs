use crate::prelude::*;

use rhai::{Engine};

/// SDF
#[derive(PartialEq, Debug, Clone)]
pub struct Camera {
    pub origin              : F3,
    pub center              : F3,
    pub fov                 : F,
}

impl Camera {

    pub fn new() -> Self {
        Self {
            origin          : F3::new(0.0, 0.0, 3.0),
            center          : F3::zeros(),
            fov             : 70.0,
        }
    }

    /// Create a camera ray. Have todo modular later on to support iso cameras etc.
    pub fn create_ray(&self, uv: F2, cam_offset: F2, width: F, height: F) -> Ray {

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

        let ratio = width / height;

        let pixel_size = F2::new( 1.0 / width, 1.0 / height);

        let t = (fov.to_radians() * 0.5).tan();

        let half_width = F3::new(t, t, t);
        let half_height = half_width.div_f(&ratio);

        let up_vector = F3::new(0.0, 1.0, 0.0);

        let w = (origin - center).normalize();
        let u = up_vector.cross(&w);
        let v = w.cross(&u);

        let lower_left = origin - half_width * u - half_height * v - w;
        let horizontal = (u * half_width).mult_f(&2.0);
        let vertical = v * half_height.mult_f(&2.0);

        let mut rd = lower_left - origin;
        rd += horizontal.mult_f(&(pixel_size.x * cam_offset.x + uv.x));
        rd += vertical.mult_f(&(pixel_size.y * cam_offset.y + uv.y));

        Ray::new(origin, rd.normalize())

    }


    // --------- Getter / Setter

    pub fn get_origin(&mut self) -> F3 {
        self.origin
    }

    pub fn set_origin(&mut self, new_val: F3) {
        self.origin = new_val;
    }

    pub fn get_center(&mut self) -> F3 {
        self.center
    }

    pub fn set_center(&mut self, new_val: F3) {
        self.center = new_val;
    }

    pub fn get_fov(&mut self) -> F {
        self.fov
    }

    pub fn set_fov(&mut self, new_val: F) {
        self.fov = new_val;
    }

    /// Register to the engine
    pub fn register(engine: &mut Engine) {
        engine.register_type_with_name::<Camera>("Camera")
            .register_fn("Camera", Camera::new)
            .register_get_set("origin", Camera::get_origin, Camera::set_origin)
            .register_get_set("center", Camera::get_center, Camera::set_center)
            .register_get_set("fov", Camera::get_fov, Camera::set_fov);
    }

}