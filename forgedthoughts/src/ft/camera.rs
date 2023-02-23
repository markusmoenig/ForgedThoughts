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