use crate::prelude::*;

#[derive(PartialEq, Debug, Clone)]
pub enum LightType {
    Point,
}

/// SDF
#[derive(PartialEq, Debug, Clone)]
pub struct Light {
    pub light_type          : LightType,
    pub position            : F3,
    pub radius              : F,

    pub rgb                 : F3,
    pub intensity           : F,
}

impl Light {

    pub fn new_point_light() -> Self {
        Self {
            light_type      : LightType::Point,
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
}