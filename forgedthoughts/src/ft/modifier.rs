use crate::prelude::*;

use rhai::{Engine};

#[derive(PartialEq, Debug, Clone)]
pub enum ModifierType {
    Position,
}

/// SDF
#[derive(PartialEq, Debug, Clone)]
pub struct Modifier {
    pub modifier_type       : ModifierType,
    pub position            : F3,
    pub radius              : F,

    pub rgb                 : F3,
    pub intensity           : F,
}

impl Modifier {

    pub fn new_point_light() -> Self {
        Self {
            modifier_type   : ModifierType::Position,
            position        : F3::zeros(),
            radius          : 1.0,

            rgb             : F3::new(1.0, 1.0, 1.0),
            intensity       : 1.0,
        }
    }

    // --------- Getter / Setter

    pub fn get_position(&mut self) -> F3 {
        self.position
    }

    pub fn set_position(&mut self, new_val: F3) {
        self.position = new_val;
    }

    pub fn get_rgb(&mut self) -> F3 {
        self.rgb
    }

    pub fn set_rgb(&mut self, new_val: F3) {
        self.rgb = new_val;
    }

    pub fn get_radius(&mut self) -> F {
        self.radius
    }

    pub fn set_radius(&mut self, new_val: F) {
        self.radius = new_val;
    }

    pub fn get_intensity(&mut self) -> F {
        self.intensity
    }

    pub fn set_intensity(&mut self, new_val: F) {
        self.intensity = new_val;
    }

    /// Register to the engine
    pub fn register(engine: &mut Engine) {
        engine.register_type_with_name::<Light>("PointLight")
            .register_fn("PointLight", Light::new_point_light)
            .register_get_set("rgb", Light::get_rgb, Light::set_rgb)
            .register_get_set("position", Light::get_position, Light::set_position)
            .register_get_set("radius", Light::get_radius, Light::set_radius)
            .register_get_set("intensity", Light::get_intensity, Light::set_intensity);
    }
}