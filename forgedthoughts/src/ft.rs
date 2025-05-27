use rpu::prelude::*;
use rustc_hash::FxHashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use vek::{Vec2, Vec3, Vec4};
use wasmer::{Instance, Module, Store, Value};

use crate::camera::Camera;
use crate::prelude::*;
use crate::Embedded;

pub struct FT {
    pub nodes: FxHashMap<String, Node>,
}

impl Default for FT {
    fn default() -> Self {
        Self::new()
    }
}

impl FT {
    pub fn new() -> Self {
        Self {
            nodes: FxHashMap::default(),
        }
    }

    /// Allocates a render buffer of the given size.
    pub fn create_render_buffer(&self, width: usize, height: usize) -> RenderBuffer {
        RenderBuffer::new(width, height)
    }

    /// Returns if FT is using high precision (f64) instead of f32.
    pub fn high_precision(&self) -> bool {
        #[cfg(feature = "double")]
        return true;

        #[cfg(not(feature = "double"))]
        return false;
    }

    pub fn compile_nodes(&mut self) -> Result<(), String> {
        for file in Embedded::iter() {
            let name = file.as_ref().to_string();
            if let Some(file) = Embedded::get(&name) {
                if let Ok(str_slice) = std::str::from_utf8(&file.data) {
                    let source = str_slice.to_string();
                    let mut node = Node::default();
                    match node.compile(source, self.high_precision()) {
                        Ok(_) => {
                            println!("Node '{}' compiled successfully.", node.name);
                            self.nodes.insert(node.name.clone(), node);
                        }
                        Err(err) => {
                            return Err(err);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Compile the WAT source code and run the shader with the given arguments. The shader will be executed on the given buffer.
    pub fn render_2d(
        &self,
        ft: Arc<FT>,
        rpu: &RPU,
        wat: &str,
        func_name: &str,
        buffer: &mut Arc<Mutex<RenderBuffer>>,
        tile_size: (usize, usize),
    ) -> Result<Vec<Value>, String> {
        let high_precision = ft.high_precision();

        let width = buffer.lock().unwrap().width;
        let height = buffer.lock().unwrap().height;

        let tiles = rpu.create_tiles(width, height, tile_size.0, tile_size.1);

        let tiles_mutex = Arc::new(Mutex::new(tiles));

        let num_cpus = num_cpus::get();
        let _start = ft.get_time();

        let ft_arc = Arc::clone(&ft);

        // Create threads
        let mut handles = vec![];
        for _ in 0..num_cpus {
            let ft = Arc::clone(&ft_arc);

            let tiles_mutex = Arc::clone(&tiles_mutex);
            let buffer_mutex = Arc::clone(buffer);

            let handle = thread::spawn(move || {
                let mut store = Store::default();
                let mut node_modules = ft.compile_and_instantiate_nodes(&mut store);

                let mut tile_buffer = RenderBuffer::new(tile_size.0, tile_size.1);
                loop {
                    // Lock mutex to access tiles
                    let mut tiles = tiles_mutex.lock().unwrap();

                    // Check if there are remaining tiles
                    if let Some(tile) = tiles.pop() {
                        // Release mutex before processing tile
                        drop(tiles);

                        // Process tile
                        for h in 0..tile.height {
                            for w in 0..tile.width {
                                let x = tile.x + w;
                                let y = tile.y + h;

                                if x >= width || y >= height {
                                    continue;
                                }

                                let p = (*ft).pixel_at_2d(
                                    x,
                                    y,
                                    Vec2::new(width as F, height as F),
                                    &mut node_modules,
                                    &mut store,
                                );
                                tile_buffer.set(w, h, p);
                            }
                        }
                        // Save the tile buffer to the main buffer
                        buffer_mutex
                            .lock()
                            .unwrap()
                            .copy_from(tile.x, tile.y, &tile_buffer);

                        // Save thebuffer optionally to disk after each completed block.
                        if let Ok(buffer) = buffer_mutex.lock() {
                            if let Some(path) = &buffer.file_path {
                                buffer.save(path.clone());
                            }
                        }
                    } else {
                        // No remaining tiles, exit loop
                        break;
                    }
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to finish
        for handle in handles {
            handle.join().unwrap();
        }

        let _stop = ft.get_time();
        println!("Shader execution time: {:?} ms.", _stop - _start);

        Ok(vec![])
    }

    fn pixel_at_2d(
        &self,
        x: usize,
        y: usize,
        screen_size: Vec2<F>,
        modules: &mut FxHashMap<String, (Arc<Module>, Instance)>,
        store: &mut Store,
    ) -> Color {
        let mut result: Color = [0.0, 0.0, 0.0, 0.0];

        if let Some((module, instance)) = modules.get_mut("Test") {
            if let Ok(func) = instance.exports.get_function("main") {
                #[cfg(feature = "double")]
                let args = vec![
                    Value::F64(x as f64 / screen_size.x as f64),
                    Value::F64((screen_size.y as f64 - y as f64) / screen_size.y as f64),
                    Value::F64(screen_size.x as f64),
                    Value::F64(screen_size.y as f64),
                ];

                #[cfg(not(feature = "double"))]
                let args = vec![
                    Value::F32(x as f32 / screen_size.x),
                    Value::F32((screen_size.y - y as f32) / screen_size.y),
                    Value::F32(screen_size.x),
                    Value::F32(screen_size.x),
                ];

                if let Ok(values) = func.call(store, &args) {
                    #[cfg(feature = "double")]
                    let rgba = [
                        values[0].f64().unwrap(),
                        values[1].f64().unwrap(),
                        values[2].f64().unwrap(),
                        values[3].f64().unwrap(),
                    ];

                    #[cfg(not(feature = "double"))]
                    let rgba = [
                        values[0].f32().unwrap() as f64,
                        values[1].f32().unwrap() as f64,
                        values[2].f32().unwrap() as f64,
                        values[3].f32().unwrap() as f64,
                    ];

                    result[0] = rgba[0] as F;
                    result[1] = rgba[1] as F;
                    result[2] = rgba[2] as F;
                    result[3] = rgba[3] as F;
                }
            }
        }

        result
    }

    /// Render in 3D
    #[allow(clippy::too_many_arguments)]
    pub fn render_3d(
        &self,
        ft: Arc<FT>,
        rpu: &RPU,
        wat: &str,
        func_name: &str,
        buffer: &mut Arc<Mutex<RenderBuffer>>,
        tile_size: (usize, usize),
        model: Arc<ModelBuffer>,
    ) -> Result<Vec<Value>, String> {
        let high_precision = self.high_precision();

        let width = buffer.lock().unwrap().width;
        let height = buffer.lock().unwrap().height;

        let tiles = rpu.create_tiles(width, height, tile_size.0, tile_size.1);

        let tiles_mutex = Arc::new(Mutex::new(tiles));

        let num_cpus = num_cpus::get();
        let _start = self.get_time();

        let model_clone = Arc::clone(&model);

        let ft_arc = Arc::clone(&ft);

        // Create threads
        let mut handles = vec![];
        for _ in 0..num_cpus {
            let ft = Arc::clone(&ft_arc);
            let model = Arc::clone(&model_clone);

            let tiles_mutex = Arc::clone(&tiles_mutex);
            let buffer_mutex = Arc::clone(buffer);
            let fname = func_name.to_string().clone();
            let wat = wat.to_string().clone();

            let handle = thread::spawn(move || {
                let mut store = Store::default();
                let module_rc = Module::new(&store, wat);
                match module_rc {
                    Ok(module) => {
                        let import_object = RPU::create_imports(&mut store, high_precision);
                        if let Ok(instance) = Instance::new(&mut store, &module, &import_object) {
                            if let Ok(func) = instance.exports.get_function(&fname) {
                                let mut tile_buffer = RenderBuffer::new(tile_size.0, tile_size.1);
                                loop {
                                    // Lock mutex to access tiles
                                    let mut tiles = tiles_mutex.lock().unwrap();

                                    // Check if there are remaining tiles
                                    if let Some(tile) = tiles.pop() {
                                        // Release mutex before processing tile
                                        drop(tiles);
                                        // Process tile
                                        for h in 0..tile.height {
                                            for w in 0..tile.width {
                                                let x = tile.x + w;
                                                let y = tile.y + h;

                                                if x >= width || y >= height {
                                                    continue;
                                                }

                                                let p = ft.pixel_at_3d(
                                                    x,
                                                    height - y,
                                                    Vec2::new(width as F, height as F),
                                                    Arc::clone(&model),
                                                );
                                                tile_buffer.set(w, h, p);

                                                /*
                                                let args = if high_precision {
                                                    vec![
                                                        Value::F64(x as f64 / width as f64),
                                                        Value::F64(
                                                            (height as f64 - y as f64)
                                                                / height as f64,
                                                        ),
                                                        Value::F64(width as f64),
                                                        Value::F64(height as f64),
                                                    ]
                                                } else {
                                                    vec![
                                                        Value::F32(x as f32 / width as f32),
                                                        Value::F32(
                                                            (height as f32 - y as f32)
                                                                / height as f32,
                                                        ),
                                                        Value::F32(width as f32),
                                                        Value::F32(height as f32),
                                                    ]
                                                };

                                                let mut fc: Color = [0.0, 0.0, 0.0, 0.0];
                                                for i in 0..iterations {
                                                    if let Ok(gl) =
                                                        instance.exports.get_global("mem_ptr")
                                                    {
                                                        _ = gl.set(&mut store, Value::I32(32));
                                                    }
                                                    match func.call(&mut store, &args) {
                                                        Ok(values) => {
                                                            let rgba = if high_precision {
                                                                [
                                                                    values[0].f64().unwrap(),
                                                                    values[1].f64().unwrap(),
                                                                    values[2].f64().unwrap(),
                                                                    values[3].f64().unwrap(),
                                                                ]
                                                            } else {
                                                                [
                                                                    values[0].f32().unwrap() as f64,
                                                                    values[1].f32().unwrap() as f64,
                                                                    values[2].f32().unwrap() as f64,
                                                                    values[3].f32().unwrap() as f64,
                                                                ]
                                                            };
                                                            let f = 1.0 / (i as F + 1.0);
                                                            fc[0] = fc[0] * (1.0 - f)
                                                                + rgba[0] as F * f;
                                                            fc[1] = fc[1] * (1.0 - f)
                                                                + rgba[1] as F * f;
                                                            fc[2] = fc[2] * (1.0 - f)
                                                                + rgba[2] as F * f;
                                                            fc[3] = fc[3] * (1.0 - f)
                                                                + rgba[3] as F * f;
                                                        }
                                                        Err(err) => println!("{}", err),
                                                    }

                                                    // Set the final color into the local buffer
                                                    tile_buffer.set(w, h, fc);
                                                }*/
                                            }
                                        }
                                        // Save the tile buffer to the main buffer
                                        buffer_mutex.lock().unwrap().copy_from(
                                            tile.x,
                                            tile.y,
                                            &tile_buffer,
                                        );

                                        // Save thebuffer optionally to disk after each completed block.
                                        if let Ok(buffer) = buffer_mutex.lock() {
                                            if let Some(path) = &buffer.file_path {
                                                buffer.save(path.clone());
                                            }
                                        }
                                    } else {
                                        // No remaining tiles, exit loop
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    Err(err) => println!("{}", err),
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to finish
        for handle in handles {
            handle.join().unwrap();
        }

        let _stop = self.get_time();
        println!("Shader execution time: {:?} ms.", _stop - _start);

        Ok(vec![])
    }

    pub fn pixel_at_3d(
        &self,
        x: usize,
        y: usize,
        screen_size: Vec2<F>,
        model: Arc<ModelBuffer>,
    ) -> Color {
        let uv = Vec2::new(x as F / screen_size.x, y as F / screen_size.y);

        let camera = Camera::default();
        let ray = camera.create_ray(uv, Vec2::zero(), screen_size);

        if let Some(hit) = model.raymarch(&ray) {
            [hit.normal.x, hit.normal.y, hit.normal.z, 1.0]
        } else {
            [0.0, 0.0, 0.0, 0.0]
        }

        // [x as F / screen_size.x, y as F / screen_size.y, 0.0, 1.0]
    }

    /// Compile and instantiate all nodes
    pub fn compile_and_instantiate_nodes(
        &self,
        store: &mut Store,
    ) -> FxHashMap<String, (Arc<Module>, Instance)> {
        let mut result = FxHashMap::default();

        for (name, node) in &self.nodes {
            let module = Module::new(&store, &node.wat)
                .unwrap_or_else(|e| panic!("Failed to compile '{}': {}", name, e));

            let arc_module = Arc::new(module);

            let import_object = RPU::create_imports(store, self.high_precision());

            let instance = Instance::new(store, &arc_module, &import_object)
                .unwrap_or_else(|e| panic!("Failed to instantiate '{}': {}", name, e));

            result.insert(name.clone(), (arc_module, instance));
        }

        result
    }

    /// Get the current time
    pub fn get_time(&self) -> u128 {
        #[cfg(target_arch = "wasm32")]
        {
            web_sys::window().unwrap().performance().unwrap().now() as u128
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let stop = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("Time went backwards");
            stop.as_millis()
        }
    }
}

/*
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
pub mod analytical;
pub mod procedural;

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

                    //if let Some(_bg) = scope.get("background") {
                        let rc = engine.call_fn::<F3>(&mut scope, &ast, "background", ( ( Ray::new(F3::zeros(), F3::zeros()) ), ) );

                        if rc.is_err() {
                            println!("{:?}", rc.err().unwrap().to_string());
                        } else
                        if let Some(_bc) = rc.ok() {
                            settings.background_fn = true;
                        }
                    //}

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

                    let xx = x / w;
                    let yy = y / h;

                    let coord = F2::new(xx, yy);

                    let mut total = [0.0, 0.0, 0.0, 0.0];

                    for m in 0..aa {
                        for n in 0..aa {

                            let mut color = [0.0, 0.0, 0.0, ctx.settings.opacity];

                            let cam_offset = F2::new(m as F / aa_f, n as F / aa_f) - F2::new(0.5, 0.5);
                            let ray = ctx.camera.create_ray(coord, cam_offset, w, h);

                            // Hit something ?
                            if let Some(hit) = ctx.scene.raymarch(&ray, &ctx) {
                                match ctx.settings.renderer.renderer_type {
                                    RendererType::Phong => {
                                        phong(&ctx, &hit, &mut color);
                                    },
                                    RendererType::PBR => {
                                        pbr(&ctx, &hit, &mut color);
                                    }, _ => {},
                                }
                            } else {
                                if ctx.settings.background_fn {

                                    let mut s = Scope::new();

                                    if let Some(bc) = ctx.engine.call_fn::<F3>(&mut s, &ctx.ast, "background", ( ( /*F2::new(xx, yy )*/ray ), ) ).ok() {
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

                    // if let Some(texture) = &ctx.settings.texture {
                    //     if let Some(buffer) = &texture.buffer {
                    //         pub fn mix_color(a: &[F], b: &[F], v: F) -> [F; 4] {
                    //             [   (1.0 - v) * a[0] + b[0] * v,
                    //                 (1.0 - v) * a[1] + b[1] * v,
                    //                 (1.0 - v) * a[2] + b[2] * v,
                    //                 (1.0 - v) * a[3] + b[3] * v]
                    //         }

                    //         let c = buffer.at(x as usize, y as usize);
                    //         total = mix_color(&total, &c, c[3]);
                    //     }
                    // }

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
*/
