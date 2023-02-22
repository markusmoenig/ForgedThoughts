use crate::prelude::*;

pub struct HitRecord {
    pub sdf_index       : usize,

    pub distance        : F,
    pub hit_point       : F3,
    pub normal          : F3,
}