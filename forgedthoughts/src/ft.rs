use std::ops::Mul;

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

                    let coord = F2::new(xx, 1.0 - yy);

                    let aa = 2;
                    let mut total = [0.0, 0.0, 0.0, 0.0];

                    for m in 0..aa {
                        for n in 0..aa {

                            let mut color = [0.0, 0.0, 0.0, 1.0];

                            let cam_offset = F2::new(m as F / aa as F, n as F / aa as F) - F2::new(0.5, 0.5);
                            let [ro, rd] = self.create_camera_ray(coord, F3::new(0.0, 0.0, -3.0), F3::new(0.0, 0.0, 0.0), cam_offset, width as F, height as F);

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
                                        color[0] = n.x;
                                        color[1] = n.y;
                                        color[2] = n.z;
                                    }
                                    if t > t_max {
                                        break;
                                    }
                                    t += d;
                                }
                            }

                            total[0] += color[0];
                            total[1] += color[1];
                            total[2] += color[2];
                            total[3] += color[3];
                        }
                    }

                    let aa_aa = aa as F * aa as F;
                    total[0] /= aa_aa;
                    total[1] /= aa_aa;
                    total[2] /= aa_aa;
                    total[3] /= aa_aa;

                    pixel.copy_from_slice(&total);
                }
            });

        let t = self.get_time() - start;
        println!("Rendered in {} ms", t as f64);
    }

    pub fn create_camera_ray(&self, uv: F2, origin: F3, center: F3, cam_offset: F2, width: F, height: F) -> [F3; 2] {

        /*
        let ww = (center - origin).normalize();
        let uu = ww.cross(&F3::new(0.0, 1.0, 0.0)).normalize();
        let vv = uu.cross(&ww).normalize();

        let d = (uu.mult_f(&(uv.x * cam_offset.x)) + vv.mult_f(&(uv.y * cam_offset.y)) + ww.mult_f(&2.0)).normalize();

        [origin, d]
        */

        let fov : f64 = 70.0;

        let ratio = width / height;

        let pixel_size = F2::new( 1.0 / width, 1.0 / height);

        let t = (fov.to_radians() * 0.5).tan();

        let half_width = F3::new(t, t, t);
        let half_height = half_width.div_f(&ratio);

        let up_vector = F3::new(0.0, 1.0, 0.0);

        let w = (origin - center).normalize();
        let u = up_vector.cross(&w);
        let v = w.cross(&u);

        let lower_left = origin - half_width * u - half_height * v - w;
        let horizontal = (u * half_width).mult_f(&2.0);
        let vertical = v * half_height.mult_f(&2.0);

        let mut rd = lower_left - origin;
        rd += horizontal.mult_f(&(pixel_size.x * cam_offset.x + uv.x));
        rd += vertical.mult_f(&(pixel_size.y * cam_offset.y + uv.y));

        [origin, rd.normalize()]

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