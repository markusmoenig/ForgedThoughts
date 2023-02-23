use crate::prelude::*;

use rhai::{Engine, FnPtr};

#[derive(PartialEq, Debug, Clone)]
pub enum SDFType {
    Sphere,
    Plane,
    Box,
    CappedCone,
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
    pub size                : F3,
    pub radius              : F,
    pub normal              : F3,
    pub offset              : F,

    pub rounding            : F,

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
            size            : F3::new(1.0, 1.0, 1.0),
            radius          : 1.0,
            normal          : F3::zeros(),
            offset          : 0.0,

            rounding        : 0.0,

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
            size            : F3::new(1.0, 1.0, 1.0),
            radius,
            normal          : F3::zeros(),
            offset          : 0.0,

            rounding        : 0.0,

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
            size            : F3::new(1.0, 1.0, 1.0),
            radius          : 1.0,
            normal          : F3::new(0.0, 1.0, 0.0),
            offset          : 0.0,

            rounding        : 0.0,

            material        : Material::new(),
            shade           : None,
        }
    }

    pub fn new_plane_normal(normal: F3, offset: F) -> Self {
        Self {
            id              : Uuid::new_v4(),

            subtractors     : vec![],

            sdf_type        : SDFType::Plane,
            sdf_op          : SDFOp::Add,

            position        : F3::zeros(),
            size            : F3::new(1.0, 1.0, 1.0),
            radius          : 1.0,
            normal,
            offset,

            rounding        : 0.0,

            material        : Material::new(),
            shade           : None,
        }
    }

    pub fn new_box() -> Self {
        Self {
            id              : Uuid::new_v4(),

            subtractors     : vec![],

            sdf_type        : SDFType::Box,
            sdf_op          : SDFOp::Add,

            position        : F3::zeros(),
            size            : F3::new(1.0, 1.0, 1.0),
            radius          : 1.0,
            normal          : F3::new(0.0, 1.0, 0.0),
            offset          : 0.0,

            rounding        : 0.0,

            material        : Material::new(),
            shade           : None,
        }
    }

    pub fn new_box_size(size: F3) -> Self {
        Self {
            id              : Uuid::new_v4(),

            subtractors     : vec![],

            sdf_type        : SDFType::Box,
            sdf_op          : SDFOp::Add,

            position        : F3::zeros(),
            size,
            radius          : 1.0,
            normal          : F3::new(0.0, 1.0, 0.0),
            offset          : 0.0,

            rounding        : 0.0,

            material        : Material::new(),
            shade           : None,
        }
    }

    // h = offset
    // r1, r2 = normal.xy
    pub fn new_capped_cone() -> Self {
        Self {
            id              : Uuid::new_v4(),

            subtractors     : vec![],

            sdf_type        : SDFType::CappedCone,
            sdf_op          : SDFOp::Add,

            position        : F3::zeros(),
            size            : F3::new(1.0, 1.0, 1.0),
            radius          : 1.0,
            normal          : F3::new(1.0, 0.0, 0.0),
            offset          : 1.0,

            rounding        : 0.0,

            material        : Material::new(),
            shade           : None,
        }
    }

    pub fn new_capped_cone_h_r1_r2(h: F, r1: F, r2: F) -> Self {
        Self {
            id              : Uuid::new_v4(),

            subtractors     : vec![],

            sdf_type        : SDFType::CappedCone,
            sdf_op          : SDFOp::Add,

            position        : F3::zeros(),
            size            : F3::new(1.0, 1.0, 1.0),
            radius          : 1.0,
            normal          : F3::new(r1, r2, 0.0),
            offset          : h,

            rounding        : 0.0,

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
                p.dot(&self.normal) + self.offset
            },
            SDFType::Box => {
                let q = p.abs() - self.size + F3::new_x(self.rounding);
                q.max_f(&0.0).length() + q.x.max(q.y.max(q.z)).min(0.0) - self.rounding
            },
            SDFType::CappedCone => {

                let h = self.offset;
                let r1 = self.normal.x;
                let r2 = self.normal.y;

                //  vec2 q = vec2( length(p.xz), p.y );
                //   vec2 k1 = vec2(r2,h);
                //   vec2 k2 = vec2(r2-r1,2.0*h);
                //   vec2 ca = vec2(q.x-min(q.x,(q.y<0.0)?r1:r2), abs(q.y)-h);
                //   vec2 cb = q - k1 + k2*clamp( dot(k1-q,k2)/dot2(k2), 0.0, 1.0 );
                //   float s = (cb.x<0.0 && ca.y<0.0) ? -1.0 : 1.0;
                //   return s*sqrt( min(dot2(ca),dot2(cb)) );

                let q = F2::new( F2::new(p.x, p.z).length(), p.y );
                let k1 = F2::new(r2, h);
                let k2 = F2::new(r2 - r1, 2.0 * h);
                let ca = F2::new(q.x - q.x.min(if q.y < 0.0 { r1 } else { r2}), q.y.abs() - h);
                let cb = q - k1 + k2.mult_f( &((k1 - q).dot(&k2)/k2.dot(&k2) ).clamp(0.0, 1.0) );
                let s = if cb.x < 0.0 && ca.y < 0.0 { -1.0 } else { 1.0 };

                s * ca.dot(&ca).min(cb.dot(&cb)).sqrt()
            }
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

    pub fn get_normal(&mut self) -> F3 {
        self.normal
    }

    pub fn set_normal(&mut self, new_val: F3) {
        self.normal = new_val;
    }

    pub fn get_offset(&mut self) -> F {
        self.offset
    }

    pub fn set_offset(&mut self, new_val: F) {
        self.offset = new_val;
    }

    pub fn get_r1(&mut self) -> F {
        self.normal.x
    }

    pub fn set_r1(&mut self, new_val: F) {
        self.normal.x = new_val;
    }

    pub fn get_r2(&mut self) -> F {
        self.normal.y
    }

    pub fn set_r2(&mut self, new_val: F) {
        self.normal.y = new_val;
    }

    pub fn get_radius(&mut self) -> F {
        self.radius
    }

    pub fn set_radius(&mut self, new_val: F) {
        self.radius = new_val;
    }

    pub fn get_rounding(&mut self) -> F {
        self.rounding
    }

    pub fn set_rounding(&mut self, new_val: F) {
        self.rounding = new_val;
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
            .register_fn("Plane", SDF::new_plane_normal)
            .register_fn("Box", SDF::new_box)
            .register_fn("Box", SDF::new_box_size)
            .register_fn("Cone", SDF::new_capped_cone)
            .register_fn("Cone", SDF::new_capped_cone_h_r1_r2)
            .register_fn("CappedCone", SDF::new_capped_cone)
            .register_fn("CappedCone", SDF::new_capped_cone_h_r1_r2)
            .register_get_set("material", SDF::get_material, SDF::set_material)
            .register_get_set("position", SDF::get_position, SDF::set_position)
            .register_get_set("normal", SDF::get_normal, SDF::set_normal)
            .register_get_set("radius", SDF::get_radius, SDF::set_radius)
            .register_get_set("offset", SDF::get_offset, SDF::set_offset)
            .register_get_set("height", SDF::get_offset, SDF::set_offset)
            .register_get_set("r1", SDF::get_r1, SDF::set_r1)
            .register_get_set("r2", SDF::get_r2, SDF::set_r2)
            .register_get_set("rounding", SDF::get_rounding, SDF::set_rounding)
            .register_get_set("shade", SDF::get_shade, SDF::set_shade);

        engine.register_fn("-", |a: &mut SDF, b: SDF| -> SDF {
            a.subtractors.push(b.clone());
            a.clone()
        });
    }
}