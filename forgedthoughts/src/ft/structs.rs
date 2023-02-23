use crate::prelude::*;

use rhai::{Engine};

#[derive(PartialEq, Debug, Copy, Clone)]
pub struct HitRecord {
    pub sdf_index       : usize,

    pub distance        : F,
    pub hit_point       : F3,
    pub normal          : F3,

    pub material        : Material
}

impl HitRecord {
    pub fn get_material(&mut self) -> Material {
        self.material
    }

    pub fn set_material(&mut self, new_val: Material) {
        self.material = new_val;
    }

    pub fn get_hit_point(&mut self) -> F3 {
        self.hit_point
    }

    pub fn set_hit_point(&mut self, new_val: F3) {
        self.hit_point = new_val;
    }

    /// Register to the engine
    pub fn register(engine: &mut Engine) {
        engine.register_type_with_name::<HitRecord>("HitRecord")
            .register_get_set("hit_point", HitRecord::get_hit_point, HitRecord::set_hit_point)
            .register_get_set("material", HitRecord::get_material, HitRecord::set_material);
    }
}