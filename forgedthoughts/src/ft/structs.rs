use crate::prelude::*;

use rhai::{Engine};

/// HitRecord
#[derive(Debug, Clone)]
pub struct HitRecord {
    pub distance        : F,
    pub hit_point       : F3,
    pub normal          : F3,

    pub ray             : Ray,
    pub material        : Material
}

impl HitRecord {
    pub fn get_material(&mut self) -> Material {
        self.material.clone()
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

/// AABB
#[derive(Debug, Clone)]
pub struct AABB {
    pub min             : F3,
    pub max             : F3,
}

use std::ops::Index;

impl Index<usize> for AABB {
    type Output = F3;

    fn index(&self, index: usize) -> &F3 {
        if index == 0 {
            &self.min
        } else {
            &self.max
        }
    }
}
