use crate::prelude::*;

use rhai::{Engine, FnPtr};

#[derive(PartialEq, Debug, Clone)]
pub enum SDFType {
    Sphere,
    Plane,
}

#[derive(PartialEq, Debug, Clone)]
pub enum SDFOp {
    Add,
    Subtract,
}

/// SDF
#[derive(Debug, Clone)]
pub struct SDF {
    pub id                  : Uuid,

    pub subtractors         : Vec<SDF>,

    pub sdf_type            : SDFType,
    pub sdf_op              : SDFOp,

    pub position            : F3,
    pub radius              : F,

    pub normal              : F3,

    pub material            : Material,

    pub shade               : Option<FnPtr>
}

impl SDF {

    pub fn new_sphere() -> Self {
        Self {
            id              : Uuid::new_v4(),

            subtractors     : vec![],

            sdf_type        : SDFType::Sphere,
            sdf_op          : SDFOp::Add,

            position        : F3::zeros(),
            radius          : 1.0,

            normal          : F3::zeros(),

            material        : Material::new(),

            shade           : None,
        }
    }

    pub fn new_sphere_radius(radius: F) -> Self {
        Self {
            id              : Uuid::new_v4(),

            subtractors     : vec![],

            sdf_type        : SDFType::Sphere,
            sdf_op          : SDFOp::Add,

            position        : F3::zeros(),
            radius,

            normal          : F3::zeros(),

            material        : Material::new(),

            shade           : None,
        }
    }

    pub fn new_plane() -> Self {
        Self {
            id              : Uuid::new_v4(),

            subtractors     : vec![],

            sdf_type        : SDFType::Plane,
            sdf_op          : SDFOp::Add,

            position        : F3::zeros(),
            radius          : 1.0,

            normal          : F3::new(0.0, 1.0, 0.0),

            material        : Material::new(),

            shade           : None,
        }
    }

    #[inline(always)]
    pub fn distance(&self, p: F3) -> F {

        let mut dist = match self.sdf_type {
            SDFType::Sphere => {
                (p - self.position).length() - self.radius
            },
            SDFType::Plane => {
                p.dot(&self.normal)
            },
        };

        for s in &self.subtractors {
            dist = dist.max(-s.distance(p));
        }

        dist
    }

    #[inline(always)]
    pub fn normal(&self, p: F3) -> F3 {
        let scale = 0.5773 * 0.0005;
        let e = F2::new(1.0 * scale,-1.0 * scale);

        // IQs normal function

        let mut n = e.xyy().mult_f(&self.distance(p + e.xyy()));
        n += e.yyx().mult_f(&self.distance(p + e.yyx()));
        n += e.yxy().mult_f(&self.distance(p + e.yxy()));
        n += e.xxx().mult_f(&self.distance(p + e.xxx()));
        n.normalize()
    }

    // --------- Getter / Setter

    pub fn get_material(&mut self) -> Material {
        self.material
    }

    pub fn set_material(&mut self, new_val: Material) {
        self.material = new_val;
    }

    pub fn get_position(&mut self) -> F3 {
        self.position
    }

    pub fn set_position(&mut self, new_val: F3) {
        self.position = new_val;
    }

    pub fn get_radius(&mut self) -> F {
        self.radius
    }

    pub fn set_radius(&mut self, new_val: F) {
        self.radius = new_val;
    }

    pub fn get_shade(&mut self) -> FnPtr {
        if let Some(shade) = &self.shade {
            shade.clone()
        } else {
            FnPtr::new("empty_shade").ok().unwrap()
        }
    }

    pub fn set_shade(&mut self, new_val: FnPtr) {
        self.shade = Some(new_val)
    }

    /// Register to the engine
    pub fn register(engine: &mut Engine) {
        engine.register_type_with_name::<SDF>("SDF")
            .register_fn("Sphere", SDF::new_sphere)
            .register_fn("Sphere", SDF::new_sphere_radius)
            .register_fn("Plane", SDF::new_plane)
            .register_get_set("material", SDF::get_material, SDF::set_material)
            .register_get_set("position", SDF::get_position, SDF::set_position)
            .register_get_set("radius", SDF::get_radius, SDF::set_radius)
            .register_get_set("shade", SDF::get_shade, SDF::set_shade);

        engine.register_fn("-", |a: &mut SDF, b: SDF| -> SDF {
            a.subtractors.push(b.clone());
            a.clone()
        });
    }
}