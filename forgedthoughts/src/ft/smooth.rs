use crate::prelude::*;

use rhai::{Engine};

/// SDF
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