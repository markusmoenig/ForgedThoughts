use crate::prelude::*;

/// SDF
#[derive(PartialEq, Debug, Clone)]
pub struct Settings {
    pub width               : I,
    pub height              : I,
    pub antialias           : I,
    pub background          : F3,
    pub background_fn       : bool,

}

impl Settings {

    pub fn new() -> Self {
        Self {
            width           : 800,
            height          : 600,
            antialias       : 1,
            background      : F3::zeros(),
            background_fn   : false,
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
}