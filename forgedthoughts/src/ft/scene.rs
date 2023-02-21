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
}