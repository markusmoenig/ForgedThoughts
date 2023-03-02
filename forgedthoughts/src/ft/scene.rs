use crate::prelude::*;

pub use rhai::{Scope};

/// Scene
#[derive(Debug, Clone)]
pub struct Scene {

    pub sdfs            : Vec<SDF>,
    pub lights          : Vec<Light>,
}

impl Scene {

    pub fn new() -> Self {
        Self {
            sdfs        : vec![],
            lights      : vec![],
        }
    }

    /// Build the scene
    pub fn build(&mut self, scope: &Scope) {

        let mut used_up : Vec<Uuid> = vec![];

        // First collect all operations
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

        iter = scope.iter();

        for val in iter {
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
    pub fn raymarch(&self, ro: &F3, rd: &F3, ctx: &FTContext) -> Option<HitRecord> {

        let mut t = 0.0001;
        let t_max = ctx.settings.max_distance;

        let mut d = std::f64::MAX;

        let mut hit : Option<usize> = None;
        let mut closest : Option<usize> = None;

        let mut material = Material::new();
        let iso_value = 0.0001;

        // Raymarching loop
        for _i in 0..ctx.settings.steps {

            let p = *ro + rd.mult_f(&t);

            for (index, s) in self.sdfs.iter().enumerate() {

                let rc = s.distance(ctx, p, iso_value);

                // If there is a material, assign it
                if rc.1.is_some() {
                    material = rc.1.unwrap();
                }

                if rc.0 < d {
                    closest = Some(index);
                    d = rc.0;
                }
            }

            if d.abs() < iso_value {
                hit = closest;
                break;
            } else
            if t > t_max {
                break;
            }
            t += d * ctx.settings.step_size;
        }

        if let Some(hit) = hit {

            let hit_point = *ro + rd.mult_f(&t);

            let normal = self.sdfs[hit].normal(ctx, hit_point);

            let mut hit_record = HitRecord {
                sdf_index           : hit,
                distance            : t,
                hit_point,
                normal,
                material,
            };

            if let Some(shade_ptr) = &self.sdfs[hit].shade {

                // Get a pointer to the shade function if available.
                let f = move |hit: HitRecord| -> Result<Material, _> {
                    shade_ptr.call(&ctx.engine, &ctx.ast, (hit,))
                };

                if let Some(m) = f(hit_record).ok() {
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
    pub fn shadow_march(&self, ro: &F3, rd: &F3, ctx: &FTContext) -> bool{

        let mut t = 0.0001;
        let t_max = ctx.settings.max_distance;

        let mut d = std::f64::MAX;

        let mut hit : Option<usize> = None;
        let mut closest : Option<usize> = None;

        let iso_value = 0.0001;

        // Raymarching loop
        for _i in 0..ctx.settings.steps {

            let p = *ro + rd.mult_f(&t);

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