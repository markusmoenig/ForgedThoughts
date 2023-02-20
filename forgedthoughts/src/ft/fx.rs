use crate::prelude::*;

///F2
#[derive(PartialEq, Debug, Clone)]
pub struct F2 {
    pub x                   : F,
    pub y                   : F,
}

impl F2 {

    pub fn from(v: F2) -> Self {
        Self {
            x               : v.x,
            y               : v.y,
        }
    }

    pub fn zeros() -> Self {
        Self {
            x               : 0.0,
            y               : 0.0,
        }
    }

    pub fn new_x(x: F) -> Self {
        Self {
            x               : x,
            y               : x,
        }
    }

    pub fn new(x: F, y: F) -> Self {
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
    }
}

/// F3
#[derive(PartialEq, Debug, Clone)]
pub struct F3 {
    pub x                   : F,
    pub y                   : F,
    pub z                   : F,
}

impl F3 {

    pub fn from(v: F3) -> Self {
        Self {
            x               : v.x,
            y               : v.y,
            z               : v.z,
        }
    }

    pub fn zeros() -> Self {
        Self {
            x               : 0.0,
            y               : 0.0,
            z               : 0.0,
        }
    }

    pub fn new_x(x: F) -> Self {
        Self {
            x               : x,
            y               : x,
            z               : x,
        }
    }

    pub fn new(x: F, y: F, z: F) -> Self {
        Self {
            x               : x,
            y               : y,
            z               : z,
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
    }

    pub fn get_z(&mut self) -> F {
        self.z
    }

    pub fn set_z(&mut self, new_val: F) {
        self.z = new_val;
    }
}
/*
// F4
#[derive(PartialEq, Debug, Clone)]
pub struct F4 {
    pub value               : GF4
}

impl F4 {

    pub fn new(v: Vector4<F>) -> Self {
        Self {
            value           : v,
        }
    }

    pub fn new_1(x: F) -> Self {
        Self {
            value           : GF4::new(x, x, x, x)
        }
    }

    pub fn new_4(x: F, y: F, z: F, w: F) -> Self {
        Self {
            value           : GF4::new(x, y, z, w),
        }
    }

    fn get_x(&mut self) -> F {
        self.value.x
    }

    fn set_x(&mut self, new_val: F) {
        self.value.x = new_val;
    }

    fn get_y(&mut self) -> F {
        self.value.y
    }

    fn set_y(&mut self, new_val: F) {
        self.value.y = new_val;
    }

    fn get_z(&mut self) -> F {
        self.value.z
    }

    fn set_z(&mut self, new_val: F) {
        self.value.z = new_val;
    }

    fn get_w(&mut self) -> F {
        self.value.w
    }

    fn set_w(&mut self, new_val: F) {
        self.value.w = new_val;
    }
}*/