use crate::prelude::*;

use rhai::{Engine};

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
    pub step_size           : F,
    pub max_distance        : F,

    // Polygonization
    pub grid_size           : F,
    pub grid_step_size      : F,
    pub iso_value           : F,

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
            step_size       : 1.0,

            grid_size       : 1.0,
            grid_step_size  : 0.01,
            iso_value       : 0.006,

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

    pub fn get_step_size(&mut self) -> F {
        self.step_size
    }

    pub fn set_step_size(&mut self, new_val: F) {
        self.step_size = new_val;
    }

    pub fn get_max_distance(&mut self) -> F {
        self.max_distance
    }

    pub fn set_max_distance(&mut self, new_val: F) {
        self.max_distance = new_val;
    }

    pub fn get_grid_size(&mut self) -> F {
        self.grid_size
    }

    pub fn set_grid_size(&mut self, new_val: F) {
        self.grid_size = new_val;
    }

    pub fn get_grid_step_size(&mut self) -> F {
        self.grid_step_size
    }

    pub fn set_grid_step_size(&mut self, new_val: F) {
        self.grid_step_size = new_val;
    }

    pub fn get_iso_value(&mut self) -> F {
        self.iso_value
    }

    pub fn set_iso_value(&mut self, new_val: F) {
        self.iso_value = new_val;
    }

    pub fn get_renderer(&mut self) -> Renderer {
        self.renderer
    }

    pub fn set_renderer(&mut self, new_val: Renderer) {
        self.renderer = new_val;
    }

    /// Register to the engine
    pub fn register(engine: &mut Engine) {
        engine.register_type_with_name::<Settings>("Settings")
            .register_fn("Settings", Settings::new)
            .register_get_set("width", Settings::get_width, Settings::set_width)
            .register_get_set("height", Settings::get_height, Settings::set_height)
            .register_get_set("antialias", Settings::get_antialias, Settings::set_antialias)
            .register_get_set("background", Settings::get_background, Settings::set_background)
            .register_get_set("opacity", Settings::get_opacity, Settings::set_opacity)

            .register_get_set("steps", Settings::get_steps, Settings::set_steps)
            .register_get_set("step_size", Settings::get_step_size, Settings::set_step_size)

            .register_get_set("grid_size", Settings::get_grid_size, Settings::set_grid_size)
            .register_get_set("grid_step_size", Settings::get_grid_step_size, Settings::set_grid_step_size)
            .register_get_set("iso_value", Settings::get_iso_value, Settings::set_iso_value)


            .register_get_set("max_distance", Settings::get_max_distance, Settings::set_max_distance)
            .register_get_set("renderer", Settings::get_renderer, Settings::set_renderer);
    }
}