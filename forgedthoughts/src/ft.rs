pub use crate::prelude::*;
use crate::script::create_engine;

pub use rhai::{Engine, Scope, CallFnOptions};
pub mod sdf;
pub mod settings;
pub mod lights;
pub mod camera;
pub mod scene;
pub mod renderer;
pub mod structs;
pub mod math;
pub mod ray_modifier;
pub mod operators;

use rayon::{slice::ParallelSliceMut, iter::{IndexedParallelIterator, ParallelIterator}};
use std::path::PathBuf;

use rust_pathtracer::{tracer::Tracer};

pub struct FT {
    pub engine          : Option<Engine>,
}

impl FT {
    pub fn new() -> Self {

        Self {
            engine      : None,
        }
    }

    /// Compile the given script
    pub fn compile(&self, path: PathBuf, file_name: String) -> Result<FTContext, String> {
        let main_path = path.join(file_name.clone());

        if let Some(code) = std::fs::read_to_string(main_path).ok() {
            self.compile_code(code, file_name.to_string())
        } else {
            Err(format!("Error reading file `{}`", file_name))
        }
    }

    /// Compile the given script
    pub fn compile_code(&self, code: String, file_name: String) -> Result<FTContext, String> {

        let engine = crate::script::create_engine();

        let mut scope = Scope::new();

        let settings = Settings::new();
        scope.set_value("settings", settings);

        let camera = Camera::new();
        scope.set_value("camera", camera);

        let ast = engine.compile(code.as_str());

        if ast.is_err() {
            let err = ast.err().unwrap();
            Err(format!("Error in file '{}': {}", file_name, err.to_string()))
        } else
        if ast.is_ok() {
            if let Some(mut ast) = ast.ok() {

                let rc = engine.eval_ast_with_scope::<rhai::Dynamic>(&mut scope, &mut ast);

                if rc.is_ok() {

                    // Default Settings
                    let mut settings = Settings::new();
                    if let Some(mess) = scope.get_mut("settings") {
                        if let Some(sett) = mess.read_lock::<Settings>() {
                            settings = sett.clone();
                        }
                    }

                    // Default Camera
                    let mut camera = Camera::new();
                    if let Some(mess) = scope.get_mut("camera") {
                        if let Some(sett) = mess.read_lock::<Camera>() {
                            camera = sett.clone();
                        }
                    }

                    if let Some(_bc) = engine.call_fn::<F3>(&mut scope, &ast, "background", ( ( F2::new(0.0, 0.0 ) ), ) ).ok() {
                        settings.background_fn = true;
                    }

                    let mut scene = crate::ft::scene::Scene::new();
                    scene.build(&scope);

                    ast.clear_statements();

                    let mut bsdf_tracer : Option<Tracer> = None;

                    // Setup the BSDF renderer if needed
                    if settings.renderer.renderer_type == RendererType::BSDF {

                        let bsdf_ctx = FTContext {
                            engine          : create_engine(),
                            ast             : ast.clone(),
                            scope           : scope.clone(),
                            settings        : settings.clone(),
                            camera          : camera.clone(),
                            scene           : scene.clone(),

                            bsdf_tracer     :  None,
                        };

                        let bsdf_scene = Box::new(BSDFScene::new_ctx(bsdf_ctx));
                        bsdf_tracer = Some(Tracer::new(bsdf_scene))
                    }

                    Ok(FTContext {
                        engine,
                        ast,
                        scope,
                        settings,
                        camera,
                        scene,

                        bsdf_tracer
                    })
                } else {
                    let err = rc.err().unwrap();
                    Err(format!("Error in file '{}': {}", file_name, err.to_string()))
                }
            } else {
                Err("Error".to_string())
            }
        } else {
            Err("Error".to_string())
        }

    }

    /// Render the given scope
    pub fn render(&self, ctx: &mut FTContext, buffer: &mut ColorBuffer) {

        if ctx.settings.renderer.renderer_type == RendererType::BSDF {
            if let Some(tracer) = &mut ctx.bsdf_tracer {
                tracer.render(buffer);
            }
            return
        }

        let width = buffer.width;
        let height = buffer.height;

        let w = width as F;
        let h = height as F;

        let aa = ctx.settings.antialias;
        let aa_f = aa as F;

        let start = self.get_time();

        buffer.pixels
            .par_rchunks_exact_mut(width * 4)
            .enumerate()
            .for_each(|(j, line)| {
                for (i, pixel) in line.chunks_exact_mut(4).enumerate() {
                    let i = j * width + i;

                    let x = (i % width) as F;
                    let y = (i / width) as F;

                    let xx = x as F / w;
                    let yy = y as F / h;

                    let coord = F2::new(xx, yy);

                    let mut total = [0.0, 0.0, 0.0, 0.0];

                    for m in 0..aa {
                        for n in 0..aa {

                            let mut color = [0.0, 0.0, 0.0, ctx.settings.opacity];

                            let cam_offset = F2::new(m as F / aa_f, n as F / aa_f) - F2::new(0.5, 0.5);
                            let [ro, rd] = ctx.camera.create_ray(coord, cam_offset, w, h);

                            // Hit something ?
                            if let Some(hit) = ctx.scene.raymarch(&ro, &rd, &ctx) {
                                match ctx.settings.renderer.renderer_type {
                                    RendererType::Phong => {
                                        phong(&ctx, &rd, &hit, &mut color);
                                    },
                                    RendererType::PBR => {
                                        pbr(&ctx, &rd, &hit, &mut color);
                                    }, _ => {},
                                }
                            } else {
                                if ctx.settings.background_fn {

                                    let mut s = Scope::new();

                                    if let Some(bc) = ctx.engine.call_fn::<F3>(&mut s, &ctx.ast, "background", ( ( F2::new(xx, yy ) ), ) ).ok() {
                                        color[0] = bc.x;
                                        color[1] = bc.y;
                                        color[2] = bc.z;
                                    } else {
                                        color[0] = ctx.settings.background.x;
                                        color[1] = ctx.settings.background.y;
                                        color[2] = ctx.settings.background.z;
                                    }
                                } else {
                                    color[0] = ctx.settings.background.x;
                                    color[1] = ctx.settings.background.y;
                                    color[2] = ctx.settings.background.z;
                                }
                            }

                            total[0] += color[0].clamp(0.0, 1.0);
                            total[1] += color[1].clamp(0.0, 1.0);
                            total[2] += color[2].clamp(0.0, 1.0);
                            total[3] += color[3].clamp(0.0, 1.0);
                        }
                    }

                    let aa_aa = aa_f * aa_f;
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

    fn get_time(&self) -> u128 {
        let stop = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards");
            stop.as_millis()
    }


    // Polygonize into an OBJ String
    pub fn polygonize(&self, ctx: &FTContext) -> String {

        let bb_size : F = ctx.settings.grid_size;
        let step_size : F = ctx.settings.grid_step_size;

        let mut volume : Vec<F> = vec![];

        let dim = ((bb_size * 2.0) / step_size) as u32;

        println!("Generating volume data");

        let iso_value = ctx.settings.iso_value;

        for iz in 0..dim {
            let z = -bb_size + iz as F * step_size;
            for iy in 0..dim {
                let y = -bb_size + iy as F * step_size;
                for ix in 0..dim {
                    let x = -bb_size + ix as F * step_size;
                    let p = F3::new(x, y, z);
                    let d = ctx.scene.distance(ctx, p, iso_value);
                    volume.push(d);
                }
            }
        }

        println!("Triangulating normal data");

        let mut mc = MarchingCubes::new();
        mc.set_volume(volume, dim, dim, dim);
        let triangles = mc.marching_cubes(iso_value as f32);

        println!("Generated {} polygons", triangles);

        let mut obj = "".to_string();

        let mut indices = vec![];

        // Generate vertices
        for i in 0..triangles {
            let index = i * 3;
            let v = format!("v {} {} {}\n", mc.triangles[index], mc.triangles[index+1], mc.triangles[index+2]);
            obj += v.as_str();
            indices.push(i);
        }

        // Generate faces
        for i in 0..indices.len()/3 {
            let f = format!("f {} {} {}\n", indices[i*3+2]+1, indices[i*3+1]+1, indices[i*3]+1);
            obj += f.as_str();
        }

        obj
    }

}