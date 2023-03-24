use crate::prelude::*;

use rhai::{Engine, FnPtr};

/// Supported Boolean Operations
#[derive(Debug, Clone)]
pub enum Boolean {
    Addition(SDF),
    AdditionSmooth(SDF, F),
    AdditionGroove(SDF, F, F),
    Subtraction(SDF),
    SubtractionSmooth(SDF, F),
    SubtractionGroove(SDF, F, F),
    Intersection(SDF),
    IntersectionSmooth(SDF, F),
    SMin(SDF, F),
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
            AdditionGroove(other, _ra, _rb) => {
                other.id
            },
            Subtraction(other) => {
                other.id
            },
            SubtractionSmooth(other, _smoothing) => {
                other.id
            },
            SubtractionGroove(other, _ra, _rb) => {
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
    Torus,
    Cylinder,
}

/// SDFOptions
#[derive(Debug, Clone)]
pub struct SDFOptions {
    pub mirror              : B3,

    pub bbox                : F3,

    pub position            : F3,
    pub rotation            : F3,
    pub scale               : F,

    // General

    pub size                : F3,
    pub radius              : F,
    pub height              : F,


    // Plane
    pub normal              : F3,
    pub offset              : F,

    // Cone
    pub upper_radius        : F,
    pub lower_radius        : F,

    // Torus
    pub ring_radius         : F,

    pub rounding            : F,

    pub ray_modifier        : Option<FnPtr>,

    pub modifier            : Option<RayModifier>,

    pub twist               : F3,
    pub bend                : F3,

    pub onion               : F,
    pub onion_depth         : I,

    pub max                 : F3,
    pub min                 : F3,

    pub noise               : F,

    pub visible             : bool,
}

impl SDFOptions {

    pub fn new() -> Self {
        Self {
            mirror          : B3::falsed(),

            bbox            : F3::new_x(-1.0),

            position        : F3::zeros(),
            rotation        : F3::zeros(),
            scale           : 1.0,

            size            : F3::new(1.0, 1.0, 1.0),
            radius          : 1.0,
            height          : 1.0,

            // Plane
            normal          : F3::zeros(),
            offset          : 0.0,

            // Cone
            upper_radius    : 0.0,
            lower_radius    : 1.0,

            // Torus
            ring_radius     : 0.3,

            rounding        : 0.0,

            ray_modifier    : None,

            modifier        : None,

            twist           : F3::zeros(),
            bend            : F3::zeros(),

            onion           : 0.0,
            onion_depth     : 1,

            max             : F3::new(f64::MAX, f64::MAX, f64::MAX),
            min             : F3::new(f64::MIN, f64::MIN, f64::MIN),

            noise           : 0.0,

            visible         : true,
        }
    }
}

use SDFType::*;

/// SDF
#[derive(Debug, Clone)]
pub struct SDF {
    pub id                  : Uuid,

    pub booleans            : Vec<Boolean>,

    pub sdf_type            : SDFType,

    pub material            : Material,

    pub options             : SDFOptions,
}

impl SDF {

    pub fn new_sphere() -> Self {
        Self {
            id              : Uuid::new_v4(),

            booleans        : vec![],
            sdf_type        : SDFType::Sphere,
            material        : Material::new(),
            options         : SDFOptions::new(),
        }
    }

    pub fn new_sphere_radius(radius: F) -> Self {
        let mut options = SDFOptions::new();
        options.radius = radius;

        Self {
            id              : Uuid::new_v4(),

            booleans        : vec![],
            sdf_type        : SDFType::Sphere,
            material        : Material::new(),
            options
        }
    }

    pub fn new_plane() -> Self {
        let mut options = SDFOptions::new();
        options.normal = F3::new(0.0, 1.0, 0.0);

        Self {
            id              : Uuid::new_v4(),

            booleans        : vec![],
            sdf_type        : SDFType::Plane,
            material        : Material::new(),
            options
        }
    }

    pub fn new_plane_normal(normal: F3, offset: F) -> Self {
        let mut options = SDFOptions::new();
        options.normal = normal;
        options.offset = offset;

        Self {
            id              : Uuid::new_v4(),

            booleans        : vec![],
            sdf_type        : SDFType::Plane,
            material        : Material::new(),
            options
        }
    }

    pub fn new_box() -> Self {
        let options = SDFOptions::new();

        Self {
            id              : Uuid::new_v4(),

            booleans        : vec![],
            sdf_type        : SDFType::Box,
            material        : Material::new(),
            options
        }
    }

    pub fn new_box_size(size: F3) -> Self {
        let mut options = SDFOptions::new();
        options.size = size;

        Self {
            id              : Uuid::new_v4(),

            booleans        : vec![],
            sdf_type        : SDFType::Box,
            material        : Material::new(),
            options
        }
    }

    pub fn new_capped_cone() -> Self {
        let mut options = SDFOptions::new();
        options.upper_radius = 0.0;
        options.lower_radius = 1.0;
        options.height = 1.0;

        Self {
            id              : Uuid::new_v4(),

            booleans        : vec![],
            sdf_type        : SDFType::CappedCone,
            material        : Material::new(),
            options
        }
    }

    pub fn new_capped_cone_h_r1_r2(h: F, r1: F, r2: F) -> Self {
        let mut options = SDFOptions::new();
        options.upper_radius = r1;
        options.lower_radius = r2;
        options.height = h;

        Self {
            id              : Uuid::new_v4(),

            booleans        : vec![],
            sdf_type        : SDFType::CappedCone,
            material        : Material::new(),
            options
        }
    }

    pub fn new_ellipsoid() -> Self {
        let mut options = SDFOptions::new();
        options.size = F3::new(1.0, 0.5, 0.5);

        Self {
            id              : Uuid::new_v4(),

            booleans        : vec![],
            sdf_type        : SDFType::Ellipsoid,
            material        : Material::new(),
            options
        }
    }

    pub fn new_ellipsoid_size(size: F3) -> Self {
        let mut options = SDFOptions::new();
        options.size = size;

        Self {
            id              : Uuid::new_v4(),

            booleans        : vec![],
            sdf_type        : SDFType::Ellipsoid,
            material        : Material::new(),
            options
        }
    }

    pub fn new_torus() -> Self {
        let mut options = SDFOptions::new();
        options.radius = 0.7;
        options.ring_radius = 0.3;

        Self {
            id              : Uuid::new_v4(),

            booleans        : vec![],
            sdf_type        : SDFType::Torus,
            material        : Material::new(),
            options
        }
    }

    pub fn new_torus_r1r2(r1: F, r2: F) -> Self {
        let mut options = SDFOptions::new();
        options.radius = r1;
        options.ring_radius = r2;

        Self {
            id              : Uuid::new_v4(),

            booleans        : vec![],
            sdf_type        : SDFType::Torus,
            material        : Material::new(),
            options
        }
    }

    pub fn new_cylinder() -> Self {
        let mut options = SDFOptions::new();
        options.height = 1.0;
        options.radius = 0.5;

        Self {
            id              : Uuid::new_v4(),

            booleans        : vec![],
            sdf_type        : SDFType::Cylinder,
            material        : Material::new(),
            options
        }
    }

    pub fn new_cylinder_p1p2r(h: F, r: F) -> Self {
        let mut options = SDFOptions::new();
        options.height = h;
        options.radius = r;

        Self {
            id              : Uuid::new_v4(),

            booleans        : vec![],
            sdf_type        : SDFType::Cylinder,
            material        : Material::new(),
            options
        }
    }

    #[inline(always)]
    pub fn distance(&self, ctx: &FTContext, mut p: F3, iso_value: F) -> (F, Option<Material>) {

        let orig_p = p;

        // Twist

        if self.options.twist.x != 0.0 {
            #[inline(always)]
            fn op_twist_x(p: F3, k: F) -> F3 {
                let cx = f64::cos(k * p.x);
                let sx = f64::sin(k * p.x);
                let m11 = 1.0;
                let m12 = 0.0;
                let m13 = 0.0;
                let m21 = 0.0;
                let m22 = cx;
                let m23 = -sx;
                let m31 = 0.0;
                let m32 = sx;
                let m33 = cx;
                let qx = m11 * p.x + m12 * p.y + m13 * p.z;
                let qy = m21 * p.x + m22 * p.y + m23 * p.z;
                let qz = m31 * p.x + m32 * p.y + m33 * p.z;
                F3::new(qx, qy, qz)
            }
            p = op_twist_x(p, self.options.twist.x);
        }

        if self.options.twist.y != 0.0 {
            #[inline(always)]
            fn op_twist(p: F3, k: F) -> F3 {
                let cy = f64::cos(k * p.y);
                let sy = f64::sin(k * p.y);
                let m11 = cy;
                let m12 = -sy;
                let m21 = sy;
                let m22 = cy;
                let qx = m11 * p.x + m12 * p.z;
                let qz = m21 * p.x + m22 * p.z;
                let qy = p.y;
                F3::new(qx, qy, qz)
            }
            p = op_twist(p, self.options.twist.y);
        }

        if self.options.twist.z != 0.0 {
            #[inline(always)]
            fn op_twist_z(p: F3, k: F) -> F3 {
                let cz = f64::cos(k * p.z);
                let sz = f64::sin(k * p.z);
                let m11 = cz;
                let m12 = -sz;
                let m13 = 0.0;
                let m21 = sz;
                let m22 = cz;
                let m23 = 0.0;
                let m31 = 0.0;
                let m32 = 0.0;
                let m33 = 1.0;
                let qx = m11 * p.x + m12 * p.y + m13 * p.z;
                let qy = m21 * p.x + m22 * p.y + m23 * p.z;
                let qz = m31 * p.x + m32 * p.y + m33 * p.z;
                F3::new(qx, qy, qz)
            }
            p = op_twist_z(p, self.options.twist.z);
        }

        // Bend

        if self.options.bend.x != 0.0 {
            #[inline(always)]
            fn op_bend_x(p: F3, k: F) -> F3 {
                let cx = f64::cos(k * p.x);
                let sx = f64::sin(k * p.x);
                let m11 = cx;
                let m12 = -sx;
                let m21 = sx;
                let m22 = cx;
                let qx = m11 * p.x + m12 * p.y;
                let qy = m21 * p.x + m22 * p.y;
                let qz = p.z;
                F3::new(qx, qy, qz)
            }
            p = op_bend_x(p, self.options.bend.x);
        }

        if self.options.bend.y != 0.0 {
            #[inline(always)]
            fn op_bend_y(p: F3, k: F) -> F3 {
                let cx = f64::cos(k * p.x);
                let sx = f64::sin(k * p.x);
                let m11 = 1.0;
                let m12 = 0.0;
                let m13 = 0.0;
                let m21 = 0.0;
                let m22 = cx;
                let m23 = -sx;
                let m31 = 0.0;
                let m32 = sx;
                let m33 = cx;
                let qx = m11 * p.x + m12 * p.y + m13 * p.z;
                let qy = m21 * p.x + m22 * p.y + m23 * p.z;
                let qz = m31 * p.x + m32 * p.y + m33 * p.z;
                F3::new(qx, qy, qz)
            }
            p = op_bend_y(p, self.options.bend.y);
        }

        if self.options.bend.z != 0.0 {
            #[inline(always)]
            fn op_bend_z(p: F3, k: F) -> F3 {
                let cx = f64::cos(k * p.x);
                let sx = f64::sin(k * p.x);
                let m11 = cx;
                let m12 = -sx;
                let m21 = sx;
                let m22 = cx;
                let qx = m11 * p.x + m12 * p.y;
                let qy = m21 * p.x + m22 * p.y;
                let qz = p.z;
                F3::new(qx, qy, qz)
            }
            p = op_bend_z(p, self.options.bend.z);
        }

        // Mirror

        if self.options.mirror.x {
            p.x = p.x.abs();
        }
        if self.options.mirror.y {
            p.y = p.y.abs();
        }
        if self.options.mirror.z {
            p.z = p.z.abs();
        }

        p = p - self.options.position;
        p = p.div_f(&self.options.scale);

        if let Some(modifier) = self.options.modifier {
            p = modifier.generate(p);
        }

        if let Some(ray_modifier_ptr) = &self.options.ray_modifier {

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
                p.length() - self.options.radius
            },
            Plane => {
                p.dot(&self.options.normal) + self.options.offset
            },
            Box => {
                let q = p.abs() - self.options.size + F3::new_x(self.options.rounding);
                q.max_f(&0.0).length() + q.x.max(q.y.max(q.z)).min(0.0) - self.options.rounding
            },
            CappedCone => {

                let h = (self.options.height - self.options.rounding).max(0.0);
                let r1 = (self.options.lower_radius - self.options.rounding).max(0.0);
                let r2 = (self.options.upper_radius - self.options.rounding).max(0.0);

                let q = F2::new( F2::new(p.x, p.z).length(), p.y );
                let k1 = F2::new(r2, h);
                let k2 = F2::new(r2 - r1, 2.0 * h);
                let ca = F2::new(q.x - q.x.min(if q.y < 0.0 { r1 } else { r2}), q.y.abs() - h);
                let cb = q - k1 + k2.mult_f( &((k1 - q).dot(&k2)/k2.dot(&k2) ).clamp(0.0, 1.0) );
                let s = if cb.x < 0.0 && ca.y < 0.0 { -1.0 } else { 1.0 };

                s * ca.dot(&ca).min(cb.dot(&cb)).sqrt() - self.options.rounding
            },
            Ellipsoid => {
                let k0 = (p / self.options.size).length();
                let k1 = (p / (self.options.size * self.options.size)).length();
                k0 * (k0 - 1.0) / k1
            },
            Torus => {
                let q = F2::new(F2::new(p.x, p.y).length() - self.options.radius, p.z);
                q.length() - self.options.ring_radius
            },
            Cylinder => {
                let d = F2::new(F2::new(p.x, p.z).length(), p.y).abs() - F2::new(self.options.radius - self.options.rounding, self.options.height);

                (min(max(d.x, d.y), 0.0) + d.max_f(&0.0).length()) - self.options.rounding
            }
        };

        // Onion

        if self.options.onion != 0.0 {
            for _i in 0..self.options.onion_depth {
                dist = dist.abs() - self.options.onion;
            }
        }

        // Noise
        /*
        if self.noise != 0.0 {

            fn smin(a: F, b: F, k: F) -> F {
                let h = (k-(a-b).abs()).max(0.0);
                a.min(b) - h * h * 0.25 / k
            }

            fn smax(a: F, b: F, k: F) -> F {
                let h = (k-(a-b).abs()).max(0.0);
                a.max(b) + h * h * 0.25 / k
            }

            fn rad(p: [F; 3]) -> F {
                let q = [17.0 * (p[0] * 0.3183099 + 0.11).fract(),
                        17.0 * (p[1] * 0.3183099 + 0.17).fract(),
                        17.0 * (p[2] * 0.3183099 + 0.13).fract()];
                let r = (q[0] * q[1] * q[2] * (q[0] + q[1] + q[2])).fract();
                0.7 * r * r
            }

            fn noise_sdf(p: [F; 3], level: F) -> F {
                let i = [p[0].floor(), p[1].floor(), p[2].floor()];
                let f = [p[0].fract(), p[1].fract(), p[2].fract()];
                let sph = |i: [F; 3], f: [F; 3], c: [F; 3]| {
                    let l = ((f[0] - c[0]).powi(2) + (f[1] - c[1]).powi(2) + (f[2] - c[2]).powi(2)).sqrt();
                    l - rad([i[0] + c[0], i[1] + c[1], i[2] + c[2]]) * level
                };
                let s1 = sph(i, f, [0.0, 0.0, 0.0]);
                let s2 = sph(i, f, [0.0, 0.0, 1.0]);
                let s3 = sph(i, f, [0.0, 1.0, 0.0]);
                let s4 = sph(i, f, [0.0, 1.0, 1.0]);
                let s5 = sph(i, f, [1.0, 0.0, 0.0]);
                let s6 = sph(i, f, [1.0, 0.0, 1.0]);
                let s7 = sph(i, f, [1.0, 1.0, 0.0]);
                let s8 = sph(i, f, [1.0, 1.0, 1.0]);
                s1.min(s2).min(s3).min(s4).min(s5).min(s6).min(s7).min(s8)
            }

            const M: [[F; 3]; 3] = [[0.0, 1.6, 1.2],
                                    [-1.6, 0.72, -0.96],
                                    [-1.2, -0.96, 1.28]];

            let mut q = [p.x, p.y, p.z];
            let level = self.noise;
            let mut t = 0.0;
            let mut s = 1.0;
            let ioct = 11;
            for _i in 0..ioct {

                let mut n = noise_sdf(q, 1.0) * s;// * level;
                let dist1 = dist - 0.1 * s * level;
                let dist2 = 0.3 * s * level;
                n = smax(n, dist1, dist2);
                n = smin(n, dist, dist2);
                dist = n;

                t += dist;
                let [x, y, z] = q;
                q[0] = x * M[0][0] + y * M[0][1] + z * M[0][2];
                q[1] = x * M[1][0] + y * M[1][1] + z * M[1][2];
                q[2] = x * M[2][0] + y * M[2][1] + z * M[2][2];
                q[2] += -1.8 * t * s * level;
                s *= 0.415;
            }
        }*/

        // Max
        dist = dist.max(orig_p.x - self.options.max.x);
        dist = dist.max(orig_p.y - self.options.max.y);
        dist = dist.max(orig_p.z - self.options.max.z);

        // Min
        // dist = dist.min(orig_p.x - self.min.x);
        // dist = dist.min(orig_p.y - self.min.y);
        // dist = dist.min(orig_p.z - self.min.z);

        // If the distance is smaller than the is_value we automatically mix the materials

        let mut material : Option<Material> = None;

        // Assign our own material (in case there are no booleans)
        if dist < iso_value {
            material = Some(self.material.clone());
        }

        // Booleans

        for s in &self.booleans {
            match s {
                Boolean::Addition(other) => {
                    let other_hit = other.distance(ctx, p, iso_value);

                    if other_hit.0 < dist {
                        dist = other_hit.0;
                        material = other_hit.1;
                    }
                },
                Boolean::AdditionSmooth(other, smoothing) => {

                    #[inline(always)]
                    fn op_smooth_union(d1: F, d2: F, k: F) -> (F, F) {
                        let h = (0.5 + 0.5 * (d2 - d1) / k).clamp(0.0, 1.0);
                        (d2 * (1.0 - h) + d1 * h - k * h * (1.0 - h), h)
                    }

                    let other_hit = other.distance(ctx, p, iso_value);

                    let dh = op_smooth_union(other_hit.0, dist, *smoothing);

                    dist = dh.0;
                    if dist < iso_value {
                        material = Some(self.material.mix(&other.material, dh.1));
                    }
                },
                Boolean::AdditionGroove(other, ra, rb) => {
                    let other_hit = other.distance(ctx, p, iso_value);

                    let a = dist;
                    let b = other_hit.0;

                    let d = a.min((a - ra).max(b.abs() - rb));
                    if d < iso_value {
                        if d != a {
                            material = Some(other.material.clone());
                        }
                    }
                    dist = d;
                },
                Boolean::Subtraction(other) => {
                    let other_hit = other.distance(ctx, p, iso_value);

                    dist = dist.max(-other_hit.0);
                },
                Boolean::SubtractionSmooth(other, smoothing) => {

                    #[inline(always)]
                    fn op_smooth_subtraction(d1: F, d2: F, k: F) -> (F, F) {
                        let h = (0.5 - 0.5 * (d2 + d1) / k).clamp(0.0, 1.0);
                        (d2 * (1.0 - h) - d1 * h + k * h * (1.0 - h), h)
                    }

                    let other_hit = other.distance(ctx, p, iso_value);

                    let dh = op_smooth_subtraction(other_hit.0, dist, *smoothing);

                    dist = dh.0;
                    if dist < iso_value {
                        material = Some(self.material.mix(&other.material, dh.1));
                    }
                },
                Boolean::SubtractionGroove(other, ra, rb) => {
                    let other_hit = other.distance(ctx, p, iso_value);

                    let a = dist;
                    let b = other_hit.0;

	                //return max(a, min(a + ra, rb - abs(b)));

                    let d = a.max((a + ra).min(rb - b.abs()));
                    if d < iso_value {
                        if d != a {
                            material = Some(other.material.clone());
                        }
                    }
                    dist = d;
                },
                Boolean::Intersection(other) => {
                    let other_hit = other.distance(ctx, p, iso_value);

                    dist = dist.max(other_hit.0);
                },
                Boolean::IntersectionSmooth(other, smoothing) => {

                    #[inline(always)]
                    fn op_smooth_intersection(d1: F, d2: F, k: F) -> (F, F) {
                        let h = (0.5 - 0.5 * (d2 - d1) / k).clamp(0.0, 1.0);
                        (d2 * (1.0 - h) + d1 * h + k * h * (1.0 - h), h)
                    }

                    let other_hit = other.distance(ctx, p, iso_value);

                    let dh = op_smooth_intersection(other_hit.0, dist, *smoothing);

                    dist = dh.0;
                    if dist < iso_value {
                        material = Some(self.material.mix(&other.material, 1.0-dh.1));
                    }
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

                    let other_hit = other.distance(ctx, p, iso_value);

                    let a = dist;
                    let b = other_hit.0;

                    // let h = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
                    // dist = mix(b, a, h) - k * h * (1.0 - h);

                    let h = (k - (a-b).abs()).max(0.0) / k;
                    dist = a.min(b) - h * h * h * k * (1.0 / 6.0);

                    if dist < iso_value {
                        material = Some(self.material.mix(&other.material, h));
                    }
                },
            }
        }

        (dist * self.options.scale, material)
    }

    #[inline(always)]
    pub fn normal(&self, ctx: &FTContext, p: F3) -> F3 {
        let scale = 0.5773 * 0.0005;
        let e = F2::new(1.0 * scale,-1.0 * scale);
        let iso_value = 0.0;

        // IQs normal function

        let mut n = e.xyy().mult_f(&self.distance(ctx, p + e.xyy(), iso_value).0);
        n += e.yyx().mult_f(&self.distance(ctx, p + e.yyx(), iso_value).0);
        n += e.yxy().mult_f(&self.distance(ctx, p + e.yxy(), iso_value).0);
        n += e.xxx().mult_f(&self.distance(ctx, p + e.xxx(), iso_value).0);
        n.normalize()
    }

    /// Create an AABB for the SDF.
    pub fn create_aabb(&self) -> Option<AABB> {

        let size = self.options.bbox;

        if size.x < 0.0 {
            None
        } else {
            Some(AABB {
                min : F3::new(self.options.position.x - size.x, self.options.position.y - size.y, self.options.position.z - size.z),
                max : F3::new(self.options.position.x + size.x, self.options.position.y + size.y, self.options.position.z + size.z),
            })
        }
    }

    // --------- Getter / Setter

    pub fn copy(&mut self) -> SDF {
        let mut c = self.clone();
        c.id = Uuid::new_v4();
        c
    }

    pub fn get_material(&mut self) -> Material {
        self.material.clone()
    }

    pub fn set_material(&mut self, new_val: Material) {
        self.material = new_val;
    }

    pub fn get_bbox(&mut self) -> F3 {
        self.options.bbox
    }

    pub fn set_bbox(&mut self, new_val: F3) {
        self.options.bbox = new_val;
    }

    pub fn get_position(&mut self) -> F3 {
        self.options.position
    }

    pub fn set_position(&mut self, new_val: F3) {
        self.options.position = new_val;
    }

    pub fn get_rotation(&mut self) -> F3 {
        self.options.rotation
    }

    pub fn set_rotation(&mut self, new_val: F3) {
        self.options.rotation = new_val;
    }

    pub fn get_scale(&mut self) -> F {
        self.options.scale
    }

    pub fn set_scale(&mut self, new_val: F) {
        self.options.scale = new_val;
    }

    pub fn get_normal(&mut self) -> F3 {
        self.options.normal
    }

    pub fn set_normal(&mut self, new_val: F3) {
        self.options.normal = new_val;
    }

    pub fn get_mirror(&mut self) -> B3 {
        self.options.mirror
    }

    pub fn set_mirror(&mut self, new_val: B3) {
        self.options.mirror = new_val;
    }

    pub fn get_twist(&mut self) -> F3 {
        self.options.twist
    }

    pub fn set_twist(&mut self, new_val: F3) {
        self.options.twist = new_val;
    }

    pub fn get_bend(&mut self) -> F3 {
        self.options.bend
    }

    pub fn set_bend(&mut self, new_val: F3) {
        self.options.bend = new_val;
    }

    pub fn get_onion(&mut self) -> F {
        self.options.onion
    }

    pub fn set_onion(&mut self, new_val: F) {
        self.options.onion = new_val;
    }

    pub fn get_onion_layers(&mut self) -> I {
        self.options.onion_depth
    }

    pub fn set_onion_layers(&mut self, new_val: I) {
        self.options.onion_depth = new_val;
    }

    pub fn get_max(&mut self) -> F3 {
        self.options.max
    }

    pub fn set_max(&mut self, new_val: F3) {
        self.options.max = new_val;
    }

    pub fn get_min(&mut self) -> F3 {
        self.options.min
    }

    pub fn set_min(&mut self, new_val: F3) {
        self.options.min = new_val;
    }

    pub fn get_size(&mut self) -> F3 {
        self.options.size
    }

    pub fn set_size(&mut self, new_val: F3) {
        self.options.size = new_val;
    }

    pub fn get_height(&mut self) -> F {
        self.options.height
    }

    pub fn set_height(&mut self, new_val: F) {
        self.options.height = new_val;
    }

    pub fn get_offset(&mut self) -> F {
        self.options.offset
    }

    pub fn set_offset(&mut self, new_val: F) {
        self.options.offset = new_val;
    }

    pub fn get_upper_radius(&mut self) -> F {
        self.options.upper_radius
    }

    pub fn set_upper_radius(&mut self, new_val: F) {
        self.options.upper_radius = new_val;
    }

    pub fn get_lower_radius(&mut self) -> F {
        self.options.lower_radius
    }

    pub fn set_lower_radius(&mut self, new_val: F) {
        self.options.lower_radius = new_val;
    }

    pub fn get_ring_radius(&mut self) -> F {
        self.options.ring_radius
    }

    pub fn set_ring_radius(&mut self, new_val: F) {
        self.options.ring_radius = new_val;
    }

    pub fn get_radius(&mut self) -> F {
        self.options.radius
    }

    pub fn set_radius(&mut self, new_val: F) {
        self.options.radius = new_val;
    }

    pub fn get_rounding(&mut self) -> F {
        self.options.rounding
    }

    pub fn set_rounding(&mut self, new_val: F) {
        self.options.rounding = new_val;
    }

    pub fn get_ray_modifier(&mut self) -> FnPtr {
        if let Some(ray_modifier) = &self.options.ray_modifier {
            ray_modifier.clone()
        } else {
            FnPtr::new("empty").ok().unwrap()
        }
    }

    pub fn set_ray_modifier(&mut self, new_val: FnPtr) {
        self.options.ray_modifier = Some(new_val)
    }

    pub fn get_modifier(&mut self) -> RayModifier {
        if let Some(m) = self.options.modifier {
            m
        } else {
            RayModifier::new("x".into(), "*".into(), "sin".into(), "y".into())
        }
    }

    pub fn set_modifier(&mut self, new_val: RayModifier) {
        self.options.modifier = Some(new_val);
    }

    pub fn get_noise(&mut self) -> F {
        self.options.noise
    }

    pub fn set_noise(&mut self, new_val: F) {
        self.options.noise = new_val;
    }

    pub fn get_visible(&mut self) -> bool {
        self.options.visible
    }

    pub fn set_visible(&mut self, new_val: bool) {
        self.options.visible = new_val;
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
            .register_fn("Torus", SDF::new_torus)
            .register_fn("Torus", SDF::new_torus_r1r2)
            .register_fn("Cylinder", SDF::new_cylinder)
            .register_fn("Cylinder", SDF::new_cylinder_p1p2r)
            .register_fn("copy", SDF::copy)

            .register_get_set("material", SDF::get_material, SDF::set_material)

            .register_get_set("bbox", SDF::get_bbox, SDF::set_bbox)

            .register_get_set("position", SDF::get_position, SDF::set_position)
            .register_get_set("rotation", SDF::get_rotation, SDF::set_rotation)
            .register_get_set("scale", SDF::get_scale, SDF::set_scale)

            .register_get_set("normal", SDF::get_normal, SDF::set_normal)
            .register_get_set("mirror", SDF::get_mirror, SDF::set_mirror)
            .register_get_set("twist", SDF::get_twist, SDF::set_twist)
            .register_get_set("bend", SDF::get_bend, SDF::set_bend)

            .register_get_set("max", SDF::get_max, SDF::set_max)
            .register_get_set("min", SDF::get_min, SDF::set_min)

            .register_get_set("onion", SDF::get_onion, SDF::set_onion)

            .register_get_set("onion_layers", SDF::get_onion_layers, SDF::set_onion_layers)

            .register_get_set("size", SDF::get_size, SDF::set_size)
            .register_get_set("radius", SDF::get_radius, SDF::set_radius)
            .register_get_set("ring_radius", SDF::get_ring_radius, SDF::set_ring_radius)
            .register_get_set("offset", SDF::get_offset, SDF::set_offset)
            .register_get_set("height", SDF::get_height, SDF::set_height)
            .register_get_set("upper_radius", SDF::get_upper_radius, SDF::set_upper_radius)
            .register_get_set("lower_radius", SDF::get_lower_radius, SDF::set_lower_radius)
            .register_get_set("rounding", SDF::get_rounding, SDF::set_rounding)
            .register_get_set("ray_modifier", SDF::get_ray_modifier, SDF::set_ray_modifier)
            .register_get_set("modifier", SDF::get_modifier, SDF::set_modifier)

            .register_get_set("noise", SDF::get_noise, SDF::set_noise)

            .register_get_set("visible", SDF::get_visible, SDF::set_visible);

        engine.register_fn("+", |a: &mut SDF, b: SDF| -> SDF {
            a.booleans.push(Boolean::Addition(b.clone()));
            a.clone()
        });

        engine.register_fn("+", |a: &mut SDF, b: Smooth| -> SDF {
            a.booleans.push(Boolean::AdditionSmooth(b.sdf.clone(), b.smoothing));
            a.clone()
        });

        engine.register_fn("+", |a: &mut SDF, b: Groove| -> SDF {
            a.booleans.push(AdditionGroove(b.sdf.clone(), b.ra, b.rb));
            let mut c = a.clone();
            c.id = Uuid::new_v4();
            a.options.visible = false;
            c
        });

        engine.register_fn("-", |a: &mut SDF, b: SDF| -> SDF {
            a.booleans.push(Boolean::Subtraction(b.clone()));
            a.clone()
        });

        engine.register_fn("-", |a: &mut SDF, b: Smooth| -> SDF {
            a.booleans.push(Boolean::SubtractionSmooth(b.sdf.clone(), b.smoothing));
            a.clone()
        });

        engine.register_fn("-", |a: &mut SDF, b: Groove| -> SDF {
            a.booleans.push(SubtractionGroove(b.sdf.clone(), b.ra, b.rb));
            let mut c = a.clone();
            c.id = Uuid::new_v4();
            a.options.visible = false;
            c
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
            a.options.visible = false;
            c
        });

        // engine.register_fn("=", |a: &mut SDF, b: SDF| {
        //     let mut c = b.clone();
        //     c.id = Uuid::new_v4();
        //     *a = c;
        // });
    }
}