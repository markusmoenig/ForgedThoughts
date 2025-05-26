use crate::prelude::*;

use rhai::{Engine};

#[derive(PartialEq, Debug, Copy, Clone)]
pub enum RendererType {
    Phong,
    PBR,
    BSDF,
}

/// Renderer Class
#[derive(PartialEq, Debug, Copy, Clone)]
pub struct Renderer {
    pub renderer_type       : RendererType,

    // Phong
    pub ambient             : F3,
    pub specular            : F3,

    // Accumulation
    pub iterations          : I,
    pub depth               : I,
}

impl Renderer {

    pub fn new_phong() -> Self {
        Self {
            renderer_type   : RendererType::Phong,

            // Phong
            ambient         : F3::new(0.05, 0.1, 0.15),
            specular        : F3::new(1.0, 1.0, 1.0),

            iterations      : 1,
            depth           : 1,
        }
    }

    pub fn new_pbr() -> Self {
        Self {
            renderer_type   : RendererType::PBR,

            // Phong
            ambient         : F3::zeros(),
            specular        : F3::zeros(),

            iterations      : 1,
            depth           : 1,
        }
    }

    pub fn new_bsdf() -> Self {
        Self {
            renderer_type   : RendererType::BSDF,

            // Phong
            ambient         : F3::zeros(),
            specular        : F3::zeros(),

            iterations      : 100,
            depth           : 4,
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

    pub fn get_iterations(&mut self) -> I {
        self.iterations
    }

    pub fn set_iterations(&mut self, new_val: I) {
        self.iterations = new_val;
    }

    pub fn get_depth(&mut self) -> I {
        self.depth
    }

    pub fn set_depth(&mut self, new_val: I) {
        self.depth = new_val;
    }

    /// Register to the engine
    pub fn register(engine: &mut Engine) {
        engine.register_type_with_name::<Renderer>("Renderer")
            .register_fn("Phong", Renderer::new_phong)
            .register_fn("PBR", Renderer::new_pbr)
            .register_fn("BSDF", Renderer::new_bsdf)

            .register_get_set("iterations", Renderer::get_iterations, Renderer::set_iterations)
            .register_get_set("depth", Renderer::get_depth, Renderer::set_depth)

            .register_get_set("ambient", Renderer::get_ambient, Renderer::set_ambient)
            .register_get_set("specular", Renderer::get_specular, Renderer::set_specular);
    }
}