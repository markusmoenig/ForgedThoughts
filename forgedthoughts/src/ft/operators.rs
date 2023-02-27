use crate::prelude::*;

use rhai::{Engine};

/// Smooth(ing) operator for -=, +=, &=
#[derive(Debug, Clone)]
pub struct Smooth {
    pub sdf                 : SDF,
    pub smoothing           : F,
}

impl Smooth {

    pub fn new(sdf: SDF, smoothing: F) -> Self {
        Self {
            sdf,
            smoothing,
        }
    }

    // --------- Getter / Setter

    pub fn set_smoothing(&mut self, new_val: F) {
        self.smoothing = new_val;
    }

    pub fn get_smoothing(&mut self) -> F {
        self.smoothing
    }

    /// Register to the engine
    pub fn register(engine: &mut Engine) {
        engine.register_type_with_name::<Smooth>("Smooth")
            .register_fn("Smooth", Smooth::new)
            .register_get_set("smoothing", Smooth::get_smoothing, Smooth::set_smoothing);
    }
}

/// Groove operator
#[derive(Debug, Clone)]
pub struct Groove {
    pub sdf                 : SDF,
    pub ra                  : F,
    pub rb                  : F,
}

impl Groove {

    pub fn new(sdf: SDF, ra: F, rb: F) -> Self {
        Self {
            sdf,
            ra,
            rb
        }
    }

    // --------- Getter / Setter

    // pub fn set_ra(&mut self, new_val: F) {
    //     self.smoothing = new_val;
    // }

    // pub fn get_smoothing(&mut self) -> F {
    //     self.smoothing
    // }

    /// Register to the engine
    pub fn register(engine: &mut Engine) {
        engine.register_type_with_name::<Groove>("Groove")
            .register_fn("Groove", Groove::new);
            //.register_get_set("smoothing", Smooth::get_smoothing, Smooth::set_smoothing);
    }
}