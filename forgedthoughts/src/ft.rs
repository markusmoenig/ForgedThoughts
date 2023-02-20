pub use crate::prelude::*;
pub use rhai::{Engine, Scope};
pub mod fx;
pub mod sdf3d;

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
    pub fn render(&self, scope: Scope, buffer: &mut ColorBuffer<F>) {

        let size = buffer.size.clone();
        let [width, height] = buffer.size;

        const LINES: usize = 20;
        let ratio = width as F / height as F;

        // let num_objects = self.bvh_nodes.len();
        // println!("num {}", num_objects);

        buffer.pixels
            .par_rchunks_exact_mut(width * LINES * 4)
            .enumerate()
            .for_each(|(j, line)| {
                for (i, pixel) in line.chunks_exact_mut(4).enumerate() {
                    let i = (LINES - j - 1) * width * LINES + i;
                    let x = (i % width) as F;
                    let y = (i / width) as F;

                    let xx = x as F / width as F;
                    let yy = y as F / height as F;

                    let coord = F2::new((xx - 0.5) * ratio, (1.0 - yy) - 0.5);

                    /*
                    let ray = self.camera.gen_ray(coord);
                    let mut hit = false;

                    if let Some(texture) = self.out_texture {
                        let index = self.textures[texture];
                        if let Some(c) = self.get_color(&ray,&[x as usize, y as usize], &color.size, &self.nodes[index].object, index) {
                            pixel.copy_from_slice(&c);
                        }
                        hit = true;
                    } else
                    if let Some(layout) = self.layouts.last() {
                        if let Some(c) = self.get_color(&ray,&[x as usize, y as usize], &color.size, &layout, 0) {
                            pixel.copy_from_slice(&c);
                            hit = true;
                        }
                    }

                    if hit == false {
                        let c = [0.0, 0.0, 0.0, 1.0];
                        pixel.copy_from_slice(&c);
                    }*/

                }
            });
    }

    /// Create an Rhai engine instance and register all FT types
    pub fn create_engine(&self) -> Engine {
        let mut engine = Engine::new();

        engine.register_type_with_name::<F2>("F2")
            .register_fn("F2", F2::zeros)
            .register_fn("F2", F2::new)
            .register_get_set("x", F2::get_x, F2::set_x)
            .register_get_set("y", F2::get_y, F2::set_y);

        engine.register_fn("+", |a: F2, b: F2| -> F2 {
            F2::new(a.x + b.x, a.y + b.y)
        });

        engine.register_type_with_name::<F3>("F3")
            .register_fn("F3", F3::zeros)
            .register_fn("F3", F3::new)
            .register_get_set("x", F3::get_x, F3::set_x)
            .register_get_set("y", F3::get_y, F3::set_y)
            .register_get_set("z", F3::get_z, F3::set_z);

        engine.register_fn("+", |a: F3, b: F3| -> F3 {
            F3::new(a.x + b.x, a.y + b.y, a.z + b.z)
        });

        // Sdf3D

        engine.register_type_with_name::<SdfSphere3D>("SdfSphere3D")
            .register_fn("SdfSphere3D", SdfSphere3D::new);

        engine.on_print(|x| println!("{}", x));

        engine
    }

}