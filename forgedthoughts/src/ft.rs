pub use crate::prelude::*;
pub use rhai::{Engine, Scope};
pub mod fx;
pub mod sdf;

use rayon::{slice::ParallelSliceMut, iter::{IndexedParallelIterator, ParallelIterator}};

pub struct FT {

}

impl FT {
    pub fn new() -> Self {

        Self {

        }
    }

    /// Compile the given script
    pub fn compile(&self, code: String) -> Result<Scope, String> {

        let engine = self.create_engine();

        let mut scope = Scope::new();

        let rc = engine.eval_with_scope::<rhai::Dynamic>(&mut scope, code.as_str());

        println!("{:?}", rc);

        Ok(scope)
    }

    /// Render the given scope
    pub fn render(&self, scope: Scope, buffer: &mut ColorBuffer) {

        let [width, height] = buffer.size;

        let ratio = width as F / height as F;

        let mut sdfs : Vec<SDF> = vec![];

        let iter = scope.iter();

        for val in iter {
            if val.2.type_name().ends_with("::SDF") {
                if let Some(s) = scope.get(val.0) {
                    sdfs.push(s.clone().cast::<SDF>());
                }
            }
        }

        let start = self.get_time();

        buffer.pixels
            .par_rchunks_exact_mut(width * 4)
            .enumerate()
            .for_each(|(j, line)| {
                for (i, pixel) in line.chunks_exact_mut(4).enumerate() {
                    let i = j * width + i;

                    let x = (i % width) as F;
                    let y = (i / width) as F;

                    let xx = x as F / width as F;
                    let yy = y as F / height as F;

                    let coord = F2::new((xx - 0.5) * ratio, (1.0 - yy) - 0.5);

                    let [ro, rd] = self.create_camera_ray(coord, F3::new(0.0, 0.0, -5.0), F3::zeros());

                    let mut c = [0.0, 0.0, 0.0, 1.0];
                    //let mut hit = false;

                    for s in &sdfs {
                        //println!("{:?}", s);

                        let dist = 0.0001;

                        let mut t = dist;
                        let t_max = 10.0;

                        for _i in 0..24 {
                            let p = ro + rd.mult_f(&t);
                            let d = s.distance(p);
                            if d < 0.001 {
                                let n = s.normal(p);
                                c[0] = n.x;
                                c[1] = n.y;
                                c[2] = n.z;
                            }
                            if t > t_max {
                                break;
                            }
                            t += d;
                        }
                    }

                    pixel.copy_from_slice(&c);
                }
            });

        let t = self.get_time() - start;
        println!("Rendered in {} ms", t as f64);
    }

    pub fn create_camera_ray(&self, uv: F2, origin: F3, center: F3) -> [F3; 2] {

        let ww = (center - origin).normalize();
        let uu = ww.cross(&F3::new(0.0, 1.0, 0.0)).normalize();
        let vv = uu.cross(&ww).normalize();

        let d = (uu.mult_f(&uv.x) + vv.mult_f(&uv.y) + ww.mult_f(&2.0)).normalize();

        [origin, d]
    }

    fn get_time(&self) -> u128 {
        let stop = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards");
            stop.as_millis()
    }

    /// Create an Rhai engine instance and register all FT types
    pub fn create_engine(&self) -> Engine {
        let mut engine = Engine::new();

        engine.set_fast_operators(false);

        engine.register_type_with_name::<F2>("F2")
            .register_fn("F2", F2::zeros)
            .register_fn("F2", F2::new)
            .register_fn("F2", F3::from)
            .register_fn("normalize", F2::normalize)
            .register_fn("length", F2::length)
            .register_fn("copy", F2::clone)
            .register_get_set("x", F2::get_x, F2::set_x)
            .register_get_set("y", F2::get_y, F2::set_y);

        engine.register_fn("+", |a: F2, b: F2| -> F2 {
            F2::new(a.x + b.x, a.y + b.y)
        });

        engine.register_fn("-", |a: F2, b: F2| -> F2 {
            F2::new(a.x - b.x, a.y - b.y)
        });

        engine.register_type_with_name::<F3>("F3")
            .register_fn("F3", F3::zeros)
            .register_fn("F3", F3::new)
            .register_fn("F3", F3::from)
            .register_fn("normalize", F3::normalize)
            .register_fn("length", F3::length)
            .register_fn("copy", F3::clone)
            .register_get_set("x", F3::get_x, F3::set_x)
            .register_get_set("y", F3::get_y, F3::set_y)
            .register_get_set("z", F3::get_z, F3::set_z);

        engine.register_fn("+", |a: F3, b: F3| -> F3 {
            F3::new(a.x + b.x, a.y + b.y, a.z + b.z)
        });

        engine.register_fn("-", |a: F3, b: F3| -> F3 {
            F3::new(a.x - b.x, a.y - b.y, a.z - b.z)
        });

        // Sdf3D

        engine.register_type_with_name::<SDF>("Sphere")
            .register_fn("Sphere", SDF::new_sphere);

        engine.on_print(|x| println!("{}", x));

        engine
    }

}