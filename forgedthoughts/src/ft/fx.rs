use crate::prelude::*;

use colors_transform::Color;
use rhai::{Engine};

use std::ops::Add;
use std::ops::Sub;
use std::ops::Mul;
use std::ops::Div;
use std::ops::AddAssign;

use std::iter::once;
use rhai::FuncArgs;

pub use nalgebra::*;
extern crate nalgebra_glm as glm;

///F2
#[derive(PartialEq, Debug, Copy, Clone)]
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

    /// Creates a copy
    pub fn copy(&mut self) -> F2 {
        self.clone()
    }

    /// Normalizes this vector
    pub fn normalize(&mut self) {
        let l = self.length();
        self.x /= l;
        self.y /= l;
    }

    /// Returns the length
    pub fn length(&self) -> F {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    pub fn dot(&self, other: &F2) -> F {
        self.x * other.x + self.y * other.y
    }

    pub fn mult_f(&self, other: &F) -> F2 {
        F2::new(self.x * other,
            self.y * other
        )
    }

    // Temporaries until proper implementation
    pub fn xyy(&self) -> F3 {
        F3::new(self.x, self.y, self.y)
    }

    pub fn yyx(&self) -> F3 {
        F3::new(self.y, self.y, self.x)
    }

    pub fn yxy(&self) -> F3 {
        F3::new(self.y, self.x, self.y)
    }

    pub fn xxx(&self) -> F3 {
        F3::new(self.x, self.x, self.x)
    }

    /// Register to the engine
    pub fn register(engine: &mut Engine) {
        engine.register_type_with_name::<F2>("F2")
            .register_fn("F2", F2::zeros)
            .register_fn("F2", F2::new)
            .register_fn("F2", F3::from)
            .register_fn("normalize", F2::normalize)
            .register_fn("length", F2::length)
            .register_fn("copy", F2::clone)
            .register_get_set("x", F2::get_x, F2::set_x)
            .register_get_set("y", F2::get_y, F2::set_y);

            engine.register_fn("+", |a: F2, b: F2| -> F2 {
                F2::new(a.x + b.x, a.y + b.y)
            });

            engine.register_fn("-", |a: F2, b: F2| -> F2 {
                F2::new(a.x - b.x, a.y - b.y)
            });
        }
}

impl FuncArgs for F2 {
    fn parse<C: Extend<rhai::Dynamic>>(self, container: &mut C) {
        container.extend(once(rhai::Dynamic::from(self)));
    }
}

impl Add for F2 {
    type Output = F2;

    fn add(self, other: F2) -> F2 {
        F2::new( self.x + other.x, self.y + other.y)
    }
}

impl Sub for F2 {
    type Output = F2;

    fn sub(self, other: F2) -> F2 {
        F2::new( self.x - other.x, self.y - other.y )
    }
}

impl Mul for F2 {
    type Output = F2;

    fn mul(self, other: F2) -> F2 {
        F2::new( self.x * other.x, self.y * other.y )
    }
}

impl Div for F2 {
    type Output = F2;

    fn div(self, other: F2) -> F2 {
        F2::new( self.x / other.x, self.y / other.y )
    }
}

/// F3
#[derive(PartialEq, Debug, Copy, Clone)]
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

    pub fn color(mut color: String) -> Self {

        if color.starts_with('#') {
            //println!("Color {}", value);
            let mut chars = color.chars();
            chars.next();
            color = chars.as_str().to_string();
        }

        use colors_transform::{Rgb};

        let mut x = 0.0;
        let mut y = 0.0;
        let mut z = 0.0;

        if let Some(rgb) = Rgb::from_hex_str(color.as_str()).ok() {
            x = rgb.get_red() as F / 255.0;
            y = rgb.get_green() as F / 255.0;
            z = rgb.get_blue() as F / 255.0;
        }

        Self {
            x,
            y,
            z
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

    /// Creates a copy
    pub fn copy(&mut self) -> F3 {
        self.clone()
    }

    /// Normalizes this vector
    pub fn normalize(&mut self) -> F3 {
        let l = self.length();
        self.x /= l;
        self.y /= l;
        self.z /= l;
        self.clone()
    }

    /// abs this vector
    pub fn abs(&self) -> F3 {
        F3::new(self.x.abs(), self.y.abs(), self.z.abs())
    }

    /// Returns the length
    pub fn length(&self) -> F {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    pub fn dot(&self, other: &F3) -> F {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    pub fn cross(&self, other: &F3) -> F3 {
        F3::new(self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x
        )
    }

    pub fn mult_f(&self, other: &F) -> F3 {
        F3::new(self.x * other,
            self.y * other,
            self.z * other
        )
    }

    pub fn div_f(&self, other: &F) -> F3 {
        F3::new(self.x / other,
            self.y / other,
            self.z / other
        )
    }

    pub fn max_f(&self, other: &F) -> F3 {
        F3::new(self.x.max(*other), self.y.max(*other), self.z.max(*other))
    }

    // Compatibility to glm

    pub fn from_v3(v: &glm::DVec3) -> Self {
        Self {
            x           : v.x,
            y           : v.y,
            z           : v.z
        }
    }

    pub fn to_v3(&self) -> glm::DVec3 {
        glm::DVec3::new(self.x, self.y, self.z)
    }

    /// Register to the engine
    pub fn register(engine: &mut Engine) {
        engine.register_type_with_name::<F3>("F3")
            .register_fn("F3", F3::zeros)
            .register_fn("F3", F3::new)
            .register_fn("F3", F3::new_x)
            .register_fn("F3", F3::from)
            .register_fn("F3", F3::color)
            .register_fn("normalize", F3::normalize)
            .register_fn("length", F3::length)
            .register_fn("copy", F3::clone)
            .register_get_set("x", F3::get_x, F3::set_x)
            .register_get_set("y", F3::get_y, F3::set_y)
            .register_get_set("z", F3::get_z, F3::set_z);

        engine.register_fn("+", |a: F3, b: F3| -> F3 {
            F3::new(a.x + b.x, a.y + b.y, a.z + b.z)
        });

        engine.register_fn("-", |a: F3, b: F3| -> F3 {
            F3::new(a.x - b.x, a.y - b.y, a.z - b.z)
        });
    }
}

impl Add for F3 {
    type Output = F3;

    fn add(self, other: F3) -> F3 {
        F3::new( self.x + other.x, self.y + other.y, self.z + other.z )
    }
}

impl AddAssign for F3 {
    fn add_assign(&mut self, other: F3) {
        self.x += other.x;
        self.y += other.y;
        self.z += other.z;
    }
}

impl Sub for F3 {
    type Output = F3;

    fn sub(self, other: F3) -> F3 {
        F3::new( self.x - other.x, self.y - other.y, self.z - other.z )
    }
}

impl Mul for F3 {
    type Output = F3;

    fn mul(self, other: F3) -> F3 {
        F3::new( self.x * other.x, self.y * other.y, self.z * other.z )
    }
}

impl Div for F3 {
    type Output = F3;

    fn div(self, other: F3) -> F3 {
        F3::new( self.x / other.x, self.y / other.y, self.z / other.z )
    }
}

/// B3
#[derive(PartialEq, Debug, Copy, Clone)]
pub struct B3 {
    pub x                   : bool,
    pub y                   : bool,
    pub z                   : bool,
}

impl B3 {

    pub fn from(v: B3) -> Self {
        Self {
            x               : v.x,
            y               : v.y,
            z               : v.z,
        }
    }

    pub fn falsed() -> Self {
        Self {
            x               : false,
            y               : false,
            z               : false,
        }
    }

    pub fn new_x(x: bool) -> Self {
        Self {
            x               : x,
            y               : x,
            z               : x,
        }
    }

    pub fn new(x: bool, y: bool, z: bool) -> Self {
        Self {
            x               : x,
            y               : y,
            z               : z,
        }
    }

    pub fn get_x(&mut self) -> bool {
        self.x
    }

    pub fn set_x(&mut self, new_val: bool) {
        self.x = new_val;
    }

    pub fn get_y(&mut self) -> bool {
        self.y
    }

    pub fn set_y(&mut self, new_val: bool) {
        self.y = new_val;
    }

    pub fn get_z(&mut self) -> bool {
        self.z
    }

    pub fn set_z(&mut self, new_val: bool) {
        self.z = new_val;
    }

    /// Register to the engine
    pub fn register(engine: &mut Engine) {
        engine.register_type_with_name::<B3>("B3")
            .register_fn("B3", B3::falsed)
            .register_fn("B3", B3::new)
            .register_fn("B3", B3::from)
            .register_get_set("x", B3::get_x, B3::set_x)
            .register_get_set("y", B3::get_y, B3::set_y)
            .register_get_set("z", B3::get_z, B3::set_z);
    }
}