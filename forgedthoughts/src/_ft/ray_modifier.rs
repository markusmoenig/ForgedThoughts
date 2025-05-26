use crate::prelude::*;

use rhai::{Engine};

/// The operation of the RayModifier
#[derive(PartialEq, Debug, Copy, Clone)]
pub enum RayModifierOp {
    Multiply,
    Division
}

/// The operation of the RayModifier
#[derive(PartialEq, Debug, Copy, Clone)]
pub enum RayModifierType {
    Sin,
    Cos,
}

/// The components of the RayModifier
#[derive(PartialEq, Debug, Copy, Clone)]
pub enum RayModifierComponent {
    X,
    Y,
    Z,
}

use RayModifierOp::*;
use RayModifierType::*;
use RayModifierComponent::*;

/// The type of the
#[derive(PartialEq, Debug, Copy, Clone)]
pub struct RayModifier {

    pub op                  : RayModifierOp,
    pub op_type             : RayModifierType,

    pub comp_modify         : RayModifierComponent,
    pub comp_with           : RayModifierComponent,

    pub frequency           : F,
    pub amplitude           : F,
    pub addend              : F,
}

//use RayModifierType::*;

impl RayModifier {

    pub fn new(modify: String, op_string : String, op_type_string: String, with: String) -> Self {

        let comp_modify = match modify.to_lowercase().as_str() {
            "y" => Y,
            "z" => Z,
            _ => X,
        };

        let op = match op_string.as_str() {
            _ => Multiply,
        };

        let op_type = match op_type_string.to_lowercase().as_str() {
            "cos" => Cos,
            _ => Sin
        };

        let comp_with = match with.to_lowercase().as_str() {
            "y" => Y,
            "z" => Z,
            _ => X,
        };

        Self {
            op,
            op_type,

            comp_modify,
            comp_with,

            frequency       : 1.0,
            amplitude       : 1.0,
            addend          : 0.0,
        }
    }

    pub fn generate(&self, mut p: F3) -> F3 {

        // Generate interior

        let mut interior = match self.comp_with {
            X => {
                p.x
            },
            Y => {
                p.y
            },
            Z => {
                p.z
            }
        };

        interior = match self.op_type {
            Sin => (interior * self.frequency).sin(),
            Cos => (interior * self.frequency).cos(),
        };

        interior *= self.amplitude;
        interior += self.addend;

        match self.op {
            Multiply => {
                match self.comp_modify {
                    X => p.x *= interior,
                    Y => p.y *= interior,
                    Z => p.z *= interior,
                }
            },
            Division => {
                match self.comp_modify {
                    X => p.x /= interior,
                    Y => p.y /= interior,
                    Z => p.z /= interior,
                }
            }
        }

        p
    }

    // --------- Getter / Setter

    pub fn get_frequency(&mut self) -> F {
        self.frequency
    }

    pub fn set_frequency(&mut self, new_val: F) {
        self.frequency = new_val;
    }

    pub fn get_amplitude(&mut self) -> F {
        self.amplitude
    }

    pub fn set_amplitude(&mut self, new_val: F) {
        self.amplitude = new_val;
    }

    pub fn get_addend(&mut self) -> F {
        self.addend
    }

    pub fn set_addend(&mut self, new_val: F) {
        self.addend = new_val;
    }

    pub fn get_op(&mut self) -> String {
        match  self.op {
            Multiply => "*".to_owned(),
            Division => "/".to_owned()
        }
    }

    pub fn set_op(&mut self, new_val: String) {
        self.op = match new_val.as_str() {
            "/" => Division,
            _ => { Multiply }
        }
    }

    pub fn get_type(&mut self) -> String {
        match  self.op_type {
            Sin => "sin".to_owned(),
            Cos => "cos".to_owned()
        }
    }

    pub fn set_type(&mut self, new_val: String) {
        self.op_type = match new_val.as_str() {
            "sin" => Sin,
            _ => { Sin }
        }
    }

    /*
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
    }*/

    /// Register to the engine
    pub fn register(engine: &mut Engine) {
        engine.register_type_with_name::<RayModifier>("RayModifier")
            .register_fn("RayModifier", RayModifier::new)
            .register_get_set("op", RayModifier::get_op, RayModifier::set_op)
            .register_get_set("type", RayModifier::get_type, RayModifier::set_type)
            .register_get_set("frequency", RayModifier::get_frequency, RayModifier::set_frequency)
            .register_get_set("amplitude", RayModifier::get_amplitude, RayModifier::set_amplitude)

            .register_get_set("addend", RayModifier::get_addend, RayModifier::set_addend);
    }
}