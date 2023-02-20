use crate::prelude::*;

/// SDF3D_Sphere
#[derive(PartialEq, Debug, Clone)]
pub struct SdfSphere3D {
    pub position            : F3,
    pub radius              : F,
}

impl SdfSphere3D {

    pub fn new() -> Self {
        Self {
            position        : F3::empty(),
            radius          : 1.0,
        }
    }

    /*
    pub fn new_1(x: F) -> Self {
        Self {
            x               : x,
            y               : x,
        }
    }

    pub fn new_2(x: F, y: F) -> Self {
        Self {
            x,
            y,
        }
    }

    pub fn get_x(&mut self) -> F {
        self.x
    }

    pub fn set_x(&mut self, new_val: F) {
        self.x = new_val;
    }

    pub fn get_y(&mut self) -> F {
        self.y
    }

    pub fn set_y(&mut self, new_val: F) {
        self.y = new_val;
    }*/
}