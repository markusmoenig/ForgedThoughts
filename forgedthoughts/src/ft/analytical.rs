use crate::prelude::*;

use rhai::{Engine, FnPtr};

/// Supported SDF Types
#[derive(PartialEq, Debug, Clone)]
pub enum AnalyticalType {
    Sphere,
    Plane,
}

use AnalyticalType::*;

/// Analytical
#[derive(Debug, Clone)]
pub struct Analytical {
    pub id                  : Uuid,

    pub analytical_type     : AnalyticalType,

    pub position            : F3,
    pub rotation            : F3,
    pub scale               : F,

    pub size                : F3,
    pub radius              : F,
    pub normal              : F3,

    pub offset              : F,

    pub material            : Material,
    pub shade               : Option<FnPtr>,

    pub visible             : bool,
}

impl Analytical {

    pub fn new_sphere() -> Self {
        Self {
            id              : Uuid::new_v4(),

            analytical_type : Sphere,

            position        : F3::zeros(),
            rotation        : F3::zeros(),
            scale           : 1.0,

            size            : F3::new(1.0, 1.0, 1.0),
            radius          : 1.0,
            normal          : F3::zeros(),

            offset          : 0.0,

            material        : Material::new(),
            shade           : None,

            visible         : true,
        }
    }

    pub fn new_sphere_radius(radius: F) -> Self {
        Self {
            id              : Uuid::new_v4(),

            analytical_type : Sphere,

            position        : F3::zeros(),
            rotation        : F3::zeros(),
            scale           : 1.0,

            size            : F3::new(1.0, 1.0, 1.0),
            radius          : radius,
            normal          : F3::zeros(),
            offset          : 0.0,

            material        : Material::new(),
            shade           : None,

            visible         : true,
        }
    }

    pub fn new_plane() -> Self {
        Self {
            id              : Uuid::new_v4(),

            analytical_type : Plane,

            position        : F3::zeros(),
            rotation        : F3::zeros(),
            scale           : 1.0,

            size            : F3::new(1.0, 1.0, 1.0),
            radius          : 1.0,
            normal          : F3::new(0.0, 1.0, 0.0),

            offset          : 0.0,

            material        : Material::new(),
            shade           : None,

            visible         : true,
        }
    }

    pub fn new_plane_normal(normal: F3) -> Self {
        Self {
            id              : Uuid::new_v4(),

            analytical_type : Plane,

            position        : F3::zeros(),
            rotation        : F3::zeros(),
            scale           : 1.0,

            size            : F3::new(1.0, 1.0, 1.0),
            radius          : 1.0,
            normal,
            offset          : 0.0,

            material        : Material::new(),
            shade           : None,

            visible         : true,
        }
    }

    #[inline(always)]
    pub fn distance(&self, _ctx: &FTContext, ray: &Ray) -> Option<(F, Material, F3)> {

        let mut hit = false;
        let mut d = F::MAX;
        let mut normal = F3::zeros();

        match self.analytical_type {
            Sphere => {
                fn sphere(ray: &Ray, center: F3, radius: F) -> Option<F> {
                    let l = center - ray.origin;
                    let tca = l.dot(&ray.direction);
                    let d2 = l.dot(&l) - tca * tca;
                    let radius2 = radius * radius;
                    if d2 > radius2 {
                        return None;
                    }
                    let thc = (radius2 - d2).sqrt();
                    let mut t0 = tca - thc;
                    let mut t1 = tca + thc;

                    if t0 > t1 {
                        std::mem::swap(&mut t0, &mut t1);
                    }

                    if t0 < 0.0 {
                        t0 = t1;
                        if t0 < 0.0 {
                            return None;
                        }
                    }

                    Some(t0)
                }

                if let Some(dist) = sphere(ray, self.position, self.radius) {
                    hit = true;
                    d = dist;
                    let hp = ray.at(&d);
                    normal = -normalize(&(self.position - hp));
                }
            },
            Plane => {
                let denom = dot(&self.normal, &ray.direction);

                if denom.abs() > 0.0001 {
                    let t = dot(&(F3::new(0.0, -1.0, 0.0) - ray.origin), &normal) / denom;
                    if t >= 0.0 {
                        hit = true;
                        d = t;
                        normal = self.normal;
                    }
                }
                //10.0//p.dot(&self.normal) + self.offset
            }
        };

        if hit {
            Some((d, self.material.clone(), normal))
        } else {
            None
        }

    }

    // --------- Getter / Setter

    pub fn copy(&mut self) -> Analytical {
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

    pub fn get_radius(&mut self) -> F {
        self.radius
    }

    pub fn set_radius(&mut self, new_val: F) {
        self.radius = new_val;
    }

    pub fn get_normal(&mut self) -> F3 {
        self.normal
    }

    pub fn set_normal(&mut self, new_val: F3) {
        self.normal = new_val;
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

    pub fn get_visible(&mut self) -> bool {
        self.visible
    }

    pub fn set_visible(&mut self, new_val: bool) {
        self.visible = new_val;
    }

    /// Register to the engine
    pub fn register(engine: &mut Engine) {
        engine.register_type_with_name::<Analytical>("Analytical")
            .register_fn("AnalyticalSphere", Analytical::new_sphere)
            .register_fn("AnalyticalSphere", Analytical::new_sphere_radius)
            .register_fn("Plane", Analytical::new_plane)
            .register_fn("Plane", Analytical::new_plane_normal)

            .register_fn("copy", Analytical::copy)

            .register_get_set("material", Analytical::get_material, Analytical::set_material)

            .register_get_set("position", Analytical::get_position, Analytical::set_position)
            .register_get_set("rotation", Analytical::get_rotation, Analytical::set_rotation)
            .register_get_set("scale", Analytical::get_scale, Analytical::set_scale)

            .register_get_set("normal", Analytical::get_normal, Analytical::set_normal)

            .register_get_set("size", Analytical::get_size, Analytical::set_size)
            .register_get_set("radius", Analytical::get_radius, Analytical::set_radius)
            .register_get_set("offset", Analytical::get_offset, Analytical::set_offset)

            //.register_get_set("shade", Analytical::get_shade, Analytical::set_shade)

            .register_get_set("visible", Analytical::get_visible, Analytical::set_visible);
    }

}