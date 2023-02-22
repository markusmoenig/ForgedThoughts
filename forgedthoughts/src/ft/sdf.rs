use crate::prelude::*;

#[derive(PartialEq, Debug, Clone)]
pub enum SDFType {
    Container,
    Sphere,
}

#[derive(PartialEq, Debug, Clone)]
pub enum SDFOp {
    Add,
    Subtract,
}

/// SDF
#[derive(PartialEq, Debug, Clone)]
pub struct SDF {
    pub id                  : Uuid,

    pub subtractors         : Vec<SDF>,

    pub sdf_type            : SDFType,
    pub sdf_op              : SDFOp,

    pub position            : F3,
    pub radius              : F,

    pub material            : Material,
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

            material        : Material::new(F3::new(0.5, 0.5, 0.5))
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

            material        : Material::new(F3::new(0.5, 0.5, 0.5))
        }
    }

    #[inline(always)]
    pub fn distance(&self, ray_position: F3) -> F {

        let mut dist = match self.sdf_type {
            SDFType::Sphere => {
                (ray_position - self.position).length() - self.radius
            },
            _ => {
                std::f64::MAX
            }
        };

        for s in &self.subtractors {
            dist = dist.max(-s.distance(ray_position));
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

}