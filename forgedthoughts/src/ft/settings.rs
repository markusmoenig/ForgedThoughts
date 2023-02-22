use crate::prelude::*;

/// SDF
#[derive(PartialEq, Debug, Clone)]
pub struct Settings {
    pub width               : I,
    pub height              : I,
    pub antialias           : I,
    pub background          : F3,
    pub background_fn       : bool,
    pub opacity             : F,

    // Raymarching
    pub steps               : I,
    pub max_distance        : F,

    // Renderer
    pub renderer            : Renderer,
}

impl Settings {

    pub fn new() -> Self {
        Self {
            width           : 800,
            height          : 600,
            antialias       : 1,
            background      : F3::zeros(),
            background_fn   : false,
            opacity         : 1.0,

            steps           : 10000,
            max_distance    : 5.0,

            renderer        : Renderer::new_phong()
        }
    }

    // --------- Getter / Setter

    pub fn get_width(&mut self) -> I {
        self.width
    }

    pub fn set_width(&mut self, new_val: I) {
        self.width = new_val;
    }

    pub fn get_height(&mut self) -> I {
        self.height
    }

    pub fn set_height(&mut self, new_val: I) {
        self.height = new_val;
    }

    pub fn get_antialias(&mut self) -> I {
        self.antialias
    }

    pub fn set_antialias(&mut self, new_val: I) {
        self.antialias = new_val;
    }

    pub fn get_background(&mut self) -> F3 {
        self.background
    }

    pub fn set_background(&mut self, new_val: F3) {
        self.background = new_val;
    }

    pub fn get_opacity(&mut self) -> F {
        self.opacity
    }

    pub fn set_opacity(&mut self, new_val: F) {
        self.opacity = new_val;
    }

    pub fn get_steps(&mut self) -> I {
        self.steps
    }

    pub fn set_steps(&mut self, new_val: I) {
        self.steps = new_val;
    }

    pub fn get_max_distance(&mut self) -> F {
        self.max_distance
    }

    pub fn set_max_distance(&mut self, new_val: F) {
        self.max_distance = new_val;
    }

    pub fn get_renderer(&mut self) -> Renderer {
        self.renderer
    }

    pub fn set_renderer(&mut self, new_val: Renderer) {
        self.renderer = new_val;
    }
}