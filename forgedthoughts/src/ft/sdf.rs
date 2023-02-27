use crate::prelude::*;

use rhai::{Engine, FnPtr};

/// Supported Boolean Operations
#[derive(Debug, Clone)]
pub enum Boolean {
    Addition(SDF),
    AdditionSmooth(SDF, F),
    Subtract(SDF),
    SubtractSmooth(SDF, F),
    Intersection(SDF),
    IntersectionSmooth(SDF, F),
    SMin(SDF, F)
}

use Boolean::*;

impl Boolean {
    pub fn other_id(&self) -> Uuid {
        match self {
            Addition(other) => {
                other.id
            },
            AdditionSmooth(other, _smoothing) => {
                other.id
            },
            Subtract(other) => {
                other.id
            },
            SubtractSmooth(other, _smoothing) => {
                other.id
            },
            Intersection(other) => {
                other.id
            },
            IntersectionSmooth(other, _smoothing) => {
                other.id
            },
            SMin(other, _k) => {
                other.id
            }
        }
    }
}

/// Supported SDF Types
#[derive(PartialEq, Debug, Clone)]
pub enum SDFType {
    Sphere,
    Plane,
    Box,
    CappedCone,
    Ellipsoid,
}

use SDFType::*;

/// SDF
#[derive(Debug, Clone)]
pub struct SDF {
    pub id                  : Uuid,

    pub booleans            : Vec<Boolean>,

    pub sdf_type            : SDFType,

    pub mirror              : B3,

    pub position            : F3,
    pub rotation            : F3,
    pub scale               : F,

    pub size                : F3,
    pub radius              : F,
    pub normal              : F3,
    pub offset              : F,

    pub rounding            : F,

    pub material            : Material,
    pub shade               : Option<FnPtr>,
    pub ray_modifier        : Option<FnPtr>,

    pub modifier            : Option<RayModifier>,

    pub visible             : bool,
}

impl SDF {

    pub fn new_sphere() -> Self {
        Self {
            id              : Uuid::new_v4(),

            booleans        : vec![],

            sdf_type        : SDFType::Sphere,

            mirror          : B3::falsed(),

            position        : F3::zeros(),
            rotation        : F3::zeros(),
            scale           : 1.0,

            size            : F3::new(1.0, 1.0, 1.0),
            radius          : 1.0,
            normal          : F3::zeros(),
            offset          : 0.0,

            rounding        : 0.0,

            material        : Material::new(),
            shade           : None,
            ray_modifier    : None,

            modifier        : None,

            visible         : true,
        }
    }

    pub fn new_sphere_radius(radius: F) -> Self {
        Self {
            id              : Uuid::new_v4(),

            booleans        : vec![],

            sdf_type        : SDFType::Sphere,

            mirror          : B3::falsed(),

            position        : F3::zeros(),
            rotation        : F3::zeros(),
            scale           : 1.0,

            size            : F3::new(1.0, 1.0, 1.0),
            radius,
            normal          : F3::zeros(),
            offset          : 0.0,

            rounding        : 0.0,

            material        : Material::new(),
            shade           : None,
            ray_modifier    : None,

            modifier        : None,

            visible         : true,
        }
    }

    pub fn new_plane() -> Self {
        Self {
            id              : Uuid::new_v4(),

            booleans        : vec![],

            sdf_type        : SDFType::Plane,

            mirror          : B3::falsed(),

            position        : F3::zeros(),
            rotation        : F3::zeros(),
            scale           : 1.0,

            size            : F3::new(1.0, 1.0, 1.0),
            radius          : 1.0,
            normal          : F3::new(0.0, 1.0, 0.0),
            offset          : 0.0,

            rounding        : 0.0,

            material        : Material::new(),
            shade           : None,
            ray_modifier    : None,

            modifier        : None,

            visible         : true,
        }
    }

    pub fn new_plane_normal(normal: F3, offset: F) -> Self {
        Self {
            id              : Uuid::new_v4(),

            booleans        : vec![],

            sdf_type        : SDFType::Plane,

            mirror          : B3::falsed(),

            position        : F3::zeros(),
            rotation        : F3::zeros(),
            scale           : 1.0,

            size            : F3::new(1.0, 1.0, 1.0),
            radius          : 1.0,
            normal,
            offset,

            rounding        : 0.0,

            material        : Material::new(),
            shade           : None,
            ray_modifier    : None,

            modifier        : None,

            visible         : true,
        }
    }

    pub fn new_box() -> Self {
        Self {
            id              : Uuid::new_v4(),

            booleans        : vec![],

            sdf_type        : SDFType::Box,

            mirror          : B3::falsed(),

            position        : F3::zeros(),
            rotation        : F3::zeros(),
            scale           : 1.0,

            size            : F3::new(1.0, 1.0, 1.0),
            radius          : 1.0,
            normal          : F3::new(0.0, 1.0, 0.0),
            offset          : 0.0,

            rounding        : 0.0,

            material        : Material::new(),
            shade           : None,
            ray_modifier    : None,

            modifier        : None,

            visible         : true,
        }
    }

    pub fn new_box_size(size: F3) -> Self {
        Self {
            id              : Uuid::new_v4(),

            booleans        : vec![],

            sdf_type        : SDFType::Box,

            mirror          : B3::falsed(),

            position        : F3::zeros(),
            rotation        : F3::zeros(),
            scale           : 1.0,

            size,
            radius          : 1.0,
            normal          : F3::new(0.0, 1.0, 0.0),
            offset          : 0.0,

            rounding        : 0.0,

            material        : Material::new(),
            shade           : None,
            ray_modifier    : None,

            modifier        : None,

            visible         : true,
        }
    }

    // h = offset
    // r1, r2 = normal.xy
    pub fn new_capped_cone() -> Self {
        Self {
            id              : Uuid::new_v4(),

            booleans        : vec![],

            sdf_type        : SDFType::CappedCone,

            mirror          : B3::falsed(),

            position        : F3::zeros(),
            rotation        : F3::zeros(),
            scale           : 1.0,

            size            : F3::new(1.0, 1.0, 1.0),
            radius          : 1.0,
            normal          : F3::new(1.0, 0.0, 0.0),
            offset          : 1.0,

            rounding        : 0.0,

            material        : Material::new(),
            shade           : None,
            ray_modifier    : None,

            modifier        : None,

            visible         : true,
        }
    }

    pub fn new_capped_cone_h_r1_r2(h: F, r1: F, r2: F) -> Self {
        Self {
            id              : Uuid::new_v4(),

            booleans        : vec![],

            sdf_type        : SDFType::CappedCone,

            mirror          : B3::falsed(),

            position        : F3::zeros(),
            rotation        : F3::zeros(),
            scale           : 1.0,

            size            : F3::new(1.0, 1.0, 1.0),
            radius          : 1.0,
            normal          : F3::new(r1, r2, 0.0),
            offset          : h,

            rounding        : 0.0,

            material        : Material::new(),
            shade           : None,
            ray_modifier    : None,

            modifier        : None,

            visible         : true,
        }
    }

    pub fn new_ellipsoid() -> Self {
        Self {
            id              : Uuid::new_v4(),

            booleans        : vec![],

            sdf_type        : SDFType::Ellipsoid,

            mirror          : B3::falsed(),

            position        : F3::zeros(),
            rotation        : F3::zeros(),
            scale           : 1.0,

            size            : F3::new(1.0, 1.0, 1.0),
            radius          : 1.0,
            normal          : F3::new(0.0, 1.0, 0.0),
            offset          : 0.0,

            rounding        : 0.0,

            material        : Material::new(),
            shade           : None,
            ray_modifier    : None,

            modifier        : None,

            visible         : true,
        }
    }

    pub fn new_ellipsoid_size(size: F3) -> Self {
        Self {
            id              : Uuid::new_v4(),

            booleans        : vec![],

            sdf_type        : SDFType::Ellipsoid,

            mirror          : B3::falsed(),

            position        : F3::zeros(),
            rotation        : F3::zeros(),
            scale           : 1.0,

            size,
            radius          : 1.0,
            normal          : F3::new(0.0, 1.0, 0.0),
            offset          : 0.0,

            rounding        : 0.0,

            material        : Material::new(),
            shade           : None,
            ray_modifier    : None,

            modifier        : None,

            visible         : true,
        }
    }

    pub fn copy(&mut self) -> SDF {
        let mut c = self.clone();
        c.id = Uuid::new_v4();
        c
    }

    #[inline(always)]
    pub fn distance(&self, ctx: &FTContext, mut p: F3) -> F {

        if self.mirror.x {
            p.x = p.x.abs();
        }
        if self.mirror.y {
            p.y = p.y.abs();
        }
        if self.mirror.z {
            p.z = p.z.abs();
        }

        p = p - self.position;
        p = p.div_f(&self.scale);

        if let Some(modifier) = self.modifier {
            p = modifier.generate(p);
        }

        if let Some(ray_modifier_ptr) = &self.ray_modifier {

            // Get a pointer to the shade function if available.
            let f = move |position: F3| -> Result<F3, _> {
                ray_modifier_ptr.call(&ctx.engine, &ctx.ast, (position,))
            };

            if let Some(mod_p) = f(p).ok() {
                p = mod_p;
            }
        }

        let mut dist = match self.sdf_type {
            Sphere => {
                p.length() - self.radius
            },
            Plane => {
                p.dot(&self.normal) + self.offset
            },
            Box => {
                let q = p.abs() - self.size + F3::new_x(self.rounding);
                q.max_f(&0.0).length() + q.x.max(q.y.max(q.z)).min(0.0) - self.rounding
            },
            CappedCone => {

                let h = (self.offset - self.rounding).max(0.0);
                let r1 = (self.normal.x - self.rounding).max(0.0);
                let r2 = (self.normal.y - self.rounding).max(0.0);

                let q = F2::new( F2::new(p.x, p.z).length(), p.y );
                let k1 = F2::new(r2, h);
                let k2 = F2::new(r2 - r1, 2.0 * h);
                let ca = F2::new(q.x - q.x.min(if q.y < 0.0 { r1 } else { r2}), q.y.abs() - h);
                let cb = q - k1 + k2.mult_f( &((k1 - q).dot(&k2)/k2.dot(&k2) ).clamp(0.0, 1.0) );
                let s = if cb.x < 0.0 && ca.y < 0.0 { -1.0 } else { 1.0 };

                s * ca.dot(&ca).min(cb.dot(&cb)).sqrt() - self.rounding
            },
            Ellipsoid => {

                let k0 = (p / self.size).length();
                let k1 = (p / (self.size * self.size)).length();
                k0 * (k0 - 1.0) / k1
                // float k0 = length(p/r);
                // float k1 = length(p/(r*r));
                // return k0*(k0-1.0)/k1;
            }
        };

        for s in &self.booleans {
            match s {
                Boolean::Addition(other) => {
                    dist = dist.min(other.distance(ctx, p));
                },
                Boolean::AdditionSmooth(other, smoothing) => {

                    #[inline(always)]
                    fn op_smooth_union(d1: F, d2: F, k: F) -> F {
                        let h = (0.5 + 0.5 * (d2 - d1) / k).clamp(0.0, 1.0);
                        d2 * (1.0 - h) + d1 * h - k * h * (1.0 - h)
                    }

                    dist = op_smooth_union(other.distance(ctx, p), dist, *smoothing);
                },
                Boolean::Subtract(other) => {
                    dist = dist.max(-other.distance(ctx, p));
                },
                Boolean::SubtractSmooth(other, smoothing) => {

                    #[inline(always)]
                    fn op_smooth_subtraction(d1: F, d2: F, k: F) -> F {
                        let h = (0.5 - 0.5 * (d2 + d1) / k).clamp(0.0, 1.0);
                        d2 * (1.0 - h) - d1 * h + k * h * (1.0 - h)
                    }

                    dist = op_smooth_subtraction(other.distance(ctx, p), dist, *smoothing);
                },
                Boolean::Intersection(other) => {
                    dist = dist.max(other.distance(ctx, p));
                },
                Boolean::IntersectionSmooth(other, smoothing) => {

                    #[inline(always)]
                    fn op_smooth_intersection(d1: F, d2: F, k: F) -> F {
                        let h = (0.5 - 0.5 * (d2 - d1) / k).clamp(0.0, 1.0);
                        d2 * (1.0 - h) + d1 * h + k * h * (1.0 - h)
                    }


                    dist = op_smooth_intersection(other.distance(ctx, p), dist, *smoothing);
                },
                Boolean::SMin(other, k) => {
                    //float h = clamp( 0.5+0.5*(b-a)/k, 0.0, 1.0 );
                    //return mix( b, a, h ) - k*h*(1.0-h);

                    // https://iquilezles.org/articles/smin/
                    //float h = max( k-abs(a-b), 0.0 )/k;
                    //return min( a, b ) - h*h*h*k*(1.0/6.0);

                    // #[inline(always)]
                    // fn mix(x: F, y: F, a: F) -> F {
                    //     x * (1.0 - a) + y * a
                    // }

                    let a = dist; let b = other.distance(ctx, p);

                    // let h = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
                    // dist = mix(b, a, h) - k * h * (1.0 - h);

                    let h = (k - (a-b).abs()).max(0.0) / k;
                    dist = a.min(b) - h * h * h * k * (1.0 / 6.0);
                },
            }
        }

        dist * self.scale
    }

    #[inline(always)]
    pub fn normal(&self, ctx: &FTContext, p: F3) -> F3 {
        let scale = 0.5773 * 0.0005;
        let e = F2::new(1.0 * scale,-1.0 * scale);

        // IQs normal function

        let mut n = e.xyy().mult_f(&self.distance(ctx, p + e.xyy()));
        n += e.yyx().mult_f(&self.distance(ctx, p + e.yyx()));
        n += e.yxy().mult_f(&self.distance(ctx, p + e.yxy()));
        n += e.xxx().mult_f(&self.distance(ctx, p + e.xxx()));
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

    pub fn get_rotation(&mut self) -> F3 {
        self.rotation
    }

    pub fn set_rotation(&mut self, new_val: F3) {
        self.rotation = new_val;
    }

    pub fn get_scale(&mut self) -> F {
        self.scale
    }

    pub fn set_scale(&mut self, new_val: F) {
        self.scale = new_val;
    }

    pub fn get_normal(&mut self) -> F3 {
        self.normal
    }

    pub fn set_normal(&mut self, new_val: F3) {
        self.normal = new_val;
    }

    pub fn get_mirror(&mut self) -> B3 {
        self.mirror
    }

    pub fn set_mirror(&mut self, new_val: B3) {
        self.mirror = new_val;
    }

    pub fn get_size(&mut self) -> F3 {
        self.size
    }

    pub fn set_size(&mut self, new_val: F3) {
        self.size = new_val;
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

    pub fn get_ray_modifier(&mut self) -> FnPtr {
        if let Some(ray_modifier) = &self.ray_modifier {
            ray_modifier.clone()
        } else {
            FnPtr::new("empty_ray_modifier").ok().unwrap()
        }
    }

    pub fn set_ray_modifier(&mut self, new_val: FnPtr) {
        self.ray_modifier = Some(new_val)
    }

    pub fn get_modifier(&mut self) -> RayModifier {
        if let Some(m) = self.modifier {
            m
        } else {
            RayModifier::new("x".into(), "*".into(), "sin".into(), "y".into())
        }
    }

    pub fn set_modifier(&mut self, new_val: RayModifier) {
        self.modifier = Some(new_val);
    }

    pub fn get_visible(&mut self) -> bool {
        self.visible
    }

    pub fn set_visible(&mut self, new_val: bool) {
        self.visible = new_val;
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
            .register_fn("Ellipsoid", SDF::new_ellipsoid)
            .register_fn("Ellipsoid", SDF::new_ellipsoid_size)

            .register_fn("copy", SDF::copy)

            .register_get_set("material", SDF::get_material, SDF::set_material)

            .register_get_set("position", SDF::get_position, SDF::set_position)
            .register_get_set("rotation", SDF::get_rotation, SDF::set_rotation)
            .register_get_set("scale", SDF::get_scale, SDF::set_scale)

            .register_get_set("normal", SDF::get_normal, SDF::set_normal)
            .register_get_set("mirror", SDF::get_mirror, SDF::set_mirror)

            .register_get_set("size", SDF::get_size, SDF::set_size)
            .register_get_set("radius", SDF::get_radius, SDF::set_radius)
            .register_get_set("offset", SDF::get_offset, SDF::set_offset)
            .register_get_set("height", SDF::get_offset, SDF::set_offset)
            .register_get_set("r1", SDF::get_r1, SDF::set_r1)
            .register_get_set("r2", SDF::get_r2, SDF::set_r2)
            .register_get_set("rounding", SDF::get_rounding, SDF::set_rounding)
            .register_get_set("ray_modifier", SDF::get_ray_modifier, SDF::set_ray_modifier)
            .register_get_set("shade", SDF::get_shade, SDF::set_shade)
            .register_get_set("modifier", SDF::get_modifier, SDF::set_modifier)

            .register_get_set("visible", SDF::get_visible, SDF::set_visible);

        engine.register_fn("+", |a: &mut SDF, b: SDF| -> SDF {
            a.booleans.push(Boolean::Addition(b.clone()));
            a.clone()
        });

        engine.register_fn("+", |a: &mut SDF, b: Smooth| -> SDF {
            a.booleans.push(Boolean::AdditionSmooth(b.sdf.clone(), b.smoothing));
            a.clone()
        });

        engine.register_fn("-", |a: &mut SDF, b: SDF| -> SDF {
            a.booleans.push(Boolean::Subtract(b.clone()));
            a.clone()
        });

        engine.register_fn("-", |a: &mut SDF, b: Smooth| -> SDF {
            a.booleans.push(Boolean::SubtractSmooth(b.sdf.clone(), b.smoothing));
            a.clone()
        });

        engine.register_fn("&", |a: &mut SDF, b: SDF| -> SDF {
            a.booleans.push(Boolean::Intersection(b.clone()));
            a.clone()
        });

        engine.register_fn("&", |a: &mut SDF, b: Smooth| -> SDF {
            a.booleans.push(Boolean::IntersectionSmooth(b.sdf.clone(), b.smoothing));
            a.clone()
        });

        engine.register_fn("smin", |a: &mut SDF, b: SDF, k: F| -> SDF {
            a.booleans.push(SMin(b, k));
            let mut c = a.clone();
            c.id = Uuid::new_v4();
            a.visible = false;
            c
        });

        // engine.register_fn("=", |a: &mut SDF, b: SDF| {
        //     let mut c = b.clone();
        //     c.id = Uuid::new_v4();
        //     *a = c;
        // });
    }
}