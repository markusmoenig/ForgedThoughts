use crate::prelude::*;

use rhai::{Engine};

#[derive(PartialEq, Debug, Copy, Clone)]
pub enum RendererType {
    Phong,
}

/// SDF
#[derive(PartialEq, Debug, Copy, Clone)]
pub struct Renderer {
    pub renderer_type       : RendererType,

    // Phong
    pub ambient             : F3,
    pub specular            : F3,
}

impl Renderer {

    pub fn new_phong() -> Self {
        Self {
            renderer_type   : RendererType::Phong,

            // Phong
            ambient         : F3::new(0.05, 0.1, 0.15),
            specular        : F3::new(1.0, 1.0, 1.0)
        }
    }

    // --------- Getter / Setter

    pub fn get_ambient(&mut self) -> F3 {
        self.ambient
    }

    pub fn set_ambient(&mut self, new_val: F3) {
        self.ambient = new_val;
    }

    pub fn get_specular(&mut self) -> F3 {
        self.specular
    }

    pub fn set_specular(&mut self, new_val: F3) {
        self.specular = new_val;
    }

    /// Register to the engine
    pub fn register(engine: &mut Engine) {
        engine.register_type_with_name::<Renderer>("Renderer")
            .register_fn("Phong", Renderer::new_phong)
            .register_get_set("ambient", Renderer::get_ambient, Renderer::set_ambient)
            .register_get_set("specular", Renderer::get_specular, Renderer::set_specular);
    }
}