pub use crate::prelude::*;

pub use rhai::{Engine, Scope, CallFnOptions};
pub mod fx;
pub mod sdf;
pub mod material;
pub mod settings;
pub mod lights;
pub mod camera;
pub mod scene;

use rayon::{slice::ParallelSliceMut, iter::{IndexedParallelIterator, ParallelIterator}};

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
    pub fn compile(&self, code: String) -> Result<FTContext, String> {

        let engine = crate::script::create_engine();

        let mut scope = Scope::new();

        let settings = Settings::new();
        scope.set_value("settings", settings);

        let camera = Camera::new();
        scope.set_value("camera", camera);

        let ast = engine.compile(code.as_str());
        if ast.is_ok() {
            if let Some(mut ast) = ast.ok() {

                let rc = engine.eval_ast_with_scope::<rhai::Dynamic>(&mut scope, &mut ast);

                if rc.is_ok() {
                    let mut settings = Settings::new();

                    if let Some(mess) = scope.get_mut("settings") {
                        if let Some(sett) = mess.read_lock::<Settings>() {
                            settings = sett.clone();
                        }
                    }

                    let mut camera = Camera::new();

                    if let Some(mess) = scope.get_mut("camera") {
                        if let Some(sett) = mess.read_lock::<Camera>() {
                            camera = sett.clone();
                        }
                    }

                    if let Some(_bc) = engine.call_fn::<F3>(&mut scope, &ast, "background", ( ( F2::new(0.0, 0.0 ) ), ) ).ok() {
                        settings.background_fn = true;
                    }

                    let mut scene = Scene::new();
                    scene.build(&scope);

                    ast.clear_statements();

                    Ok(FTContext {
                        engine,
                        ast,
                        scope,
                        settings,
                        camera,
                        scene
                    })
                } else {
                    Err("Error".to_string())
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

        let [width, height] = buffer.size;

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

                            let mut color = [0.0, 0.0, 0.0, 1.0];

                            let cam_offset = F2::new(m as F / aa_f, n as F / aa_f) - F2::new(0.5, 0.5);
                            let [ro, rd] = self.create_camera_ray(coord, ctx.camera.origin, ctx.camera.center, cam_offset, w, h);

                            let dist = 0.0001;

                            let mut t = dist;
                            let t_max = 5.0;

                            let mut d = std::f64::MAX;

                            let mut hit : Option<usize> = None;
                            let mut closest : Option<usize> = None;

                            // Raymarching loop
                            for _i in 0..120 {

                                let p = ro + rd.mult_f(&t);

                                for (index, s) in ctx.scene.sdfs.iter().enumerate() {

                                    let new_d = s.distance(p);
                                    if new_d < d {
                                        closest = Some(index);
                                        d = new_d;
                                    }
                                }

                                if d.abs() < 0.001 {
                                    hit = closest;
                                    break;
                                } else
                                if t > t_max {
                                    break;
                                }
                                t += d;
                            }

                            // Hit something ?
                            if let Some(hit) = hit {
                                let p = ro + rd.mult_f(&t);

                                let n = ctx.scene.sdfs[hit].normal(p);
                                for l in &ctx.scene.lights {
                                    let light_dir = l.position - p;

                                    // https://www.shadertoy.com/view/XlXGDj
                                    let occ = 0.5 + 0.5 * n.y;
                                    let amb = occ.clamp(0.0, 1.0);
                                    let dif = n.dot(&light_dir).clamp(0.0, 1.0);

                                    let h = (F3::new(-rd.x, -rd.y, -rd.z) + light_dir).normalize();
                                    let spe = h.dot(&n).clamp(0.0, 1.0).powf(64.0);

                                    let ambient_color = F3::new(0.05, 0.15, 0.2);

                                    // Ambient
                                    color[0] += ambient_color.x * amb * occ;
                                    color[1] += ambient_color.y * amb * occ;
                                    color[2] += ambient_color.z * amb * occ;

                                    // Diffuse
                                    color[0] += ctx.scene.sdfs[hit].material.rgb.x * dif * l.intensity * occ;
                                    color[1] += ctx.scene.sdfs[hit].material.rgb.y * dif * l.intensity * occ;
                                    color[2] += ctx.scene.sdfs[hit].material.rgb.z * dif * l.intensity * occ;

                                    // Specular
                                    color[0] += l.rgb.x * dif * spe * occ;
                                    color[1] += l.rgb.y * dif * spe * occ;
                                    color[2] += l.rgb.z * dif * spe * occ;
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

}