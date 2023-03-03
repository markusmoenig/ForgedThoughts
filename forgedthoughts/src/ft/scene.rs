use crate::prelude::*;

pub use rhai::{Engine, Scope, FnPtr};

/// Scene
#[derive(Debug, Clone)]
pub struct Scene {

    pub analytical      : Vec<Analytical>,
    pub sdfs            : Vec<SDF>,
    pub lights          : Vec<Light>,
}

impl Scene {

    pub fn new() -> Self {
        Self {
            analytical  : vec![],
            sdfs        : vec![],
            lights      : vec![],
        }
    }

    /// Build the scene
    pub fn build(&mut self, scope: &Scope) {

        let mut used_up : Vec<Uuid> = vec![];

        // First filter all SDFs which have been used in boolean ops
        let mut iter = scope.iter();

        for val in iter {
            if val.2.type_name().ends_with("::SDF") {
                if let Some(df) = scope.get(val.0) {
                    let sdf = df.clone().cast::<SDF>();

                    for s in sdf.booleans {
                        used_up.push(s.other_id());
                    }
                    if sdf.visible == false {
                        used_up.push(sdf.id);
                    }
                }
            }
        }

        // Now collect all top level objects (except the filtered ones)

        iter = scope.iter();

        for val in iter {
            if val.2.type_name().ends_with("::Analytical") {
                if let Some(s) = scope.get(val.0) {
                    let analytical = s.clone().cast::<Analytical>();

                    self.analytical.push(analytical);
                }
            } else
            if val.2.type_name().ends_with("::SDF") {
                if let Some(s) = scope.get(val.0) {
                    let sdf = s.clone().cast::<SDF>();

                    if used_up.contains(&sdf.id) == false {
                        self.sdfs.push(sdf);
                    }
                }
            } else
            if val.2.type_name().ends_with("::Light") {
                if let Some(s) = scope.get(val.0) {
                    self.lights.push(s.clone().cast::<Light>());
                }
            }
        }

        println!("Scene contains {} top level object(s).", self.sdfs.len());
    }

    #[inline(always)]
    /// Raymarch the scene and return the
    pub fn raymarch(&self, ray: &Ray, ctx: &FTContext) -> Option<HitRecord> {

        let mut t = 0.0001;
        let t_max = ctx.settings.max_distance;

        let mut hit_point = F3::zeros();
        let mut d = std::f64::MAX;
        let mut normal = F3::zeros();

        let mut hit = false;

        let mut material = Material::new();
        let iso_value = 0.0001;

        // Analytical
        for a in &self.analytical {

            if let Some(rc) = a.distance(ctx, &ray) {
                if rc.0 < d {
                    hit = true;
                    d = rc.0;
                    material = rc.1;
                    normal = rc.2;
                    hit_point = ray.at(&d);
                }
            }
        }

        // Raymarching loop
        for _i in 0..ctx.settings.steps {

            let p = ray.at(&t);

            let mut sdf_index = 0;
            for (index, s) in self.sdfs.iter().enumerate() {

                let rc = s.distance(ctx, p, iso_value);

                // If there is a material, assign it
                if rc.1.is_some() {
                    material = rc.1.unwrap();
                }

                if rc.0 < d {
                    sdf_index = index;
                    d = rc.0;
                }
            }

            if d.abs() < iso_value {
                hit = true;
                hit_point = ray.at(&d);
                normal = self.sdfs[sdf_index].normal(ctx, hit_point);
                break;
            } else
            if t > t_max {
                break;
            }
            t += d * ctx.settings.step_size;
        }

        if hit
        {
            let mut hit_record = HitRecord {
                distance            : t,
                hit_point,
                normal,
                ray                 : *ray,
                material,
            };

            if let Some(procedural_ptr) = &hit_record.material.procedural {

                // Get a pointer to the shade function if available.
                let f = move |hit_record: HitRecord| -> Result<Material, _> {
                    procedural_ptr.call(&ctx.engine, &ctx.ast, (hit_record.clone(),))
                };

                if let Some(m) = f(hit_record.clone()).ok() {
                    hit_record.material = m;
                }
            }

            Some(hit_record)
        } else {
            None
        }
    }

    #[inline(always)]
    /// Raymarch the scene for a shadow ray
    pub fn shadow_march(&self, ray: &Ray, ctx: &FTContext) -> bool{

        let mut t = 0.0001;
        let t_max = ctx.settings.max_distance;

        let mut d = std::f64::MAX;

        let mut hit : Option<usize> = None;
        let mut closest : Option<usize> = None;

        let iso_value = 0.0001;

        // Raymarching loop
        for _i in 0..ctx.settings.steps {

            let p = ray.at(&t);

            for (index, s) in self.sdfs.iter().enumerate() {

                let new_d = s.distance(ctx, p, iso_value).0;
                if new_d < d {
                    closest = Some(index);
                    d = new_d;
                }
            }

            if d.abs() < iso_value{
                hit = closest;
                break;
            } else
            if t > t_max {
                break;
            }
            t += d * ctx.settings.step_size;
        }
        hit.is_some()
    }

    /// Returns the distance for the given position. Used for polygonization
    pub fn distance(&self, ctx: &FTContext, p: F3, iso_value: F) -> F {
        let mut d : F = std::f64::MAX;

        for s in &self.sdfs {
            let new_d = s.distance(ctx, p, iso_value).0;
            if new_d < d {
                d = new_d;
            }
        }

        d
    }
}