use crate::prelude::*;

pub use rhai::{Scope};

/// Scene
#[derive(PartialEq, Debug, Clone)]
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
                    for s in sdf.subtractors {
                        used_up.push(s.id);
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
    }

    #[inline(always)]
    /// Raymarch the scene and return the
    pub fn raymarch(&self, ro: &F3, rd: &F3, settings: &Settings, normals: bool) -> Option<HitRecord> {

        let mut t = 0.0001;
        let t_max = settings.max_distance;

        let mut d = std::f64::MAX;

        let mut hit : Option<usize> = None;
        let mut closest : Option<usize> = None;

        // Raymarching loop
        for _i in 0..settings.steps {

            let p = *ro + rd.mult_f(&t);

            for (index, s) in self.sdfs.iter().enumerate() {

                let new_d = s.distance(p);
                if new_d < d {
                    closest = Some(index);
                    d = new_d;
                }
            }

            if d.abs() < 0.0001 {
                hit = closest;
                break;
            } else
            if t > t_max {
                break;
            }
            t += d;
        }

        if let Some(hit) = hit {

            let hit_point = *ro + rd.mult_f(&t);

            let normal;

            if normals {
                normal = self.sdfs[hit].normal(hit_point);
            } else {
                normal = F3::zeros();
            }

            Some(HitRecord {
                sdf_index           : hit,
                distance            : t,
                hit_point,
                normal,
            })
        } else {
            None
        }
    }
}