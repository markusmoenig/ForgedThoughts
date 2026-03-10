use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use image::{ImageError, RgbImage};
use rayon::prelude::*;
use thiserror::Error;

use crate::{
    ColorPattern, EvalState, Material, MaterialParams, MaterialSampleInput, MediumParams,
    ObjectValue, SubsurfaceParams, Value, eval_material_function, eval_material_properties,
    eval_sdf_function, eval_sdf_zero_arg_function,
    render_api::{
        Camera, CameraKind, EnvLight, Light, PinholeCamera, PointLight, Spectrum, Vec3 as ApiVec3,
    },
};

#[path = "renderer/path.rs"]
mod path;
#[path = "renderer/ray.rs"]
mod ray;

#[derive(Debug, Clone, Copy)]
pub struct RenderOptions {
    pub width: u32,
    pub height: u32,
    pub max_steps: u32,
    pub max_dist: f32,
    pub epsilon: f32,
    pub camera_z: f32,
    pub fov_y_degrees: f32,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            width: 800,
            height: 800,
            max_steps: 128,
            max_dist: 40.0,
            epsilon: 0.001,
            camera_z: 6.0,
            fov_y_degrees: 45.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccelMode {
    Naive,
    Bvh,
    Bricks,
}

#[derive(Debug, Clone, Default)]
pub struct SceneRenderSettings {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub max_steps: Option<u32>,
    pub max_dist: Option<f32>,
    pub epsilon: Option<f32>,
    pub camera_z: Option<f32>,
    pub fov_y_degrees: Option<f32>,
    pub accel: Option<AccelMode>,
    pub trace_spp: Option<u32>,
    pub trace_bounces: Option<u32>,
    pub trace_min_spp: Option<u32>,
    pub trace_noise_threshold: Option<f32>,
}

#[derive(Debug, Clone, Copy)]
pub struct PathtraceProgress {
    pub samples_done: u32,
    pub samples_total: u32,
    pub active_pixels: u32,
    pub elapsed_ms: u128,
}

#[derive(Debug, Clone, Copy)]
pub struct RayProgress {
    pub tiles_done: u32,
    pub tiles_total: u32,
    pub elapsed_ms: u128,
}

#[derive(Debug, Clone, Copy)]
pub struct PreviewProgress {
    pub tiles_done: u32,
    pub tiles_total: u32,
    pub elapsed_ms: u128,
}

#[derive(Debug, Clone, Copy)]
pub struct PathtraceSettings {
    pub spp: u32,
    pub max_bounces: u32,
    pub preview_every: u32,
    pub min_spp: u32,
    pub noise_threshold: f32,
}

impl Default for PathtraceSettings {
    fn default() -> Self {
        Self {
            spp: 16,
            max_bounces: 4,
            preview_every: 5,
            min_spp: 8,
            noise_threshold: 0.03,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RaySettings {
    pub max_depth: u32,
    pub tile_size: u32,
    pub debug_aov: Option<RayDebugAov>,
}

impl Default for RaySettings {
    fn default() -> Self {
        Self {
            max_depth: 8,
            tile_size: 64,
            debug_aov: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RayDebugAov {
    Depth,
    Normal,
    MaterialId,
    Ior,
    Transmission,
    Fresnel,
    HitT,
}

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("no suitable scene root found (expected binding like scene/result/b/s)")]
    MissingSceneRoot,
    #[error("unsupported object type '{0}' for rendering")]
    UnsupportedObjectType(String),
    #[error("expected object value for rendering")]
    ExpectedObject,
    #[error("failed to write png: {0}")]
    Image(#[from] ImageError),
}

#[derive(Clone)]
enum SdfNode {
    Sphere {
        transform: PrimitiveTransform,
        radius: f32,
        object_id: u32,
        material_id: u32,
    },
    Box {
        transform: PrimitiveTransform,
        half_size: Vec3,
        object_id: u32,
        material_id: u32,
    },
    Cylinder {
        transform: PrimitiveTransform,
        radius: f32,
        half_height: f32,
        object_id: u32,
        material_id: u32,
    },
    Torus {
        transform: PrimitiveTransform,
        major_radius: f32,
        minor_radius: f32,
        object_id: u32,
        material_id: u32,
    },
    Custom {
        transform: PrimitiveTransform,
        runtime: Arc<CustomSdfRuntime>,
        bounds_radius: f32,
        object_id: u32,
        material_id: u32,
    },
    Union {
        lhs: Box<SdfNode>,
        rhs: Box<SdfNode>,
    },
    Subtract {
        lhs: Box<SdfNode>,
        rhs: Box<SdfNode>,
    },
    Smooth {
        base: Box<SdfNode>,
        k: f32,
    },
    Round {
        base: Box<SdfNode>,
        r: f32,
    },
}

struct CustomSdfRuntime {
    state: Arc<EvalState>,
    name: String,
}

#[derive(Clone, Copy)]
struct PrimitiveTransform {
    center: Vec3,
    rot_deg: Vec3,
}

impl PrimitiveTransform {
    const fn identity() -> Self {
        Self {
            center: Vec3::new(0.0, 0.0, 0.0),
            rot_deg: Vec3::new(0.0, 0.0, 0.0),
        }
    }
}

#[derive(Clone, Copy)]
struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

impl Vec3 {
    const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }

    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }

    fn mul(self, s: f32) -> Self {
        Self::new(self.x * s, self.y * s, self.z * s)
    }

    fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    fn dot(self, rhs: Self) -> f32 {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z
    }

    fn cross(self, rhs: Self) -> Self {
        Self::new(
            self.y * rhs.z - self.z * rhs.y,
            self.z * rhs.x - self.x * rhs.z,
            self.x * rhs.y - self.y * rhs.x,
        )
    }

    fn normalize(self) -> Self {
        let len = self.length();
        if len <= f32::EPSILON {
            return self;
        }
        self.mul(1.0 / len)
    }

    fn abs(self) -> Self {
        Self::new(self.x.abs(), self.y.abs(), self.z.abs())
    }

    fn min(self, rhs: Self) -> Self {
        Self::new(self.x.min(rhs.x), self.y.min(rhs.y), self.z.min(rhs.z))
    }

    fn max(self, rhs: Self) -> Self {
        Self::new(self.x.max(rhs.x), self.y.max(rhs.y), self.z.max(rhs.z))
    }
}

#[derive(Clone, Copy)]
struct Aabb {
    min: Vec3,
    max: Vec3,
}

impl Aabb {
    fn union(self, rhs: Self) -> Self {
        Self {
            min: self.min.min(rhs.min),
            max: self.max.max(rhs.max),
        }
    }

    fn expand(self, delta: f32) -> Self {
        let d = Vec3::new(delta, delta, delta);
        Self {
            min: self.min.sub(d),
            max: self.max.add(d),
        }
    }
}

#[derive(Clone)]
struct CompiledScene {
    root: SdfNode,
    center: Vec3,
    bounds: Aabb,
    materials: Vec<MaterialKindRt>,
    object_transforms: Vec<PrimitiveTransform>,
}

struct RenderSetup {
    state: EvalState,
    camera: CameraKind,
    lights: Vec<Box<dyn Light>>,
    path_lights: Vec<PathLight>,
    materials: Vec<MaterialKindRt>,
    object_transforms: Vec<PrimitiveTransform>,
    material_def_names: Vec<String>,
}

#[derive(Clone, Copy)]
struct RayHit {
    t: f32,
    position: Vec3,
    normal: Vec3,
    front_face: bool,
    object_id: u32,
    material_id: u32,
}

#[derive(Clone, Copy)]
struct DistanceInfo {
    distance: f32,
    object_id: u32,
    material_id: u32,
}

struct CompileContext {
    next_object_id: u32,
    default_material: MaterialKindRt,
    materials: Vec<MaterialKindRt>,
    object_transforms: Vec<PrimitiveTransform>,
}

type MaterialKindRt = Material;

impl CompileContext {
    fn new(default_material: MaterialKindRt, _state: &EvalState) -> Self {
        Self {
            next_object_id: 1,
            default_material,
            materials: vec![default_material],
            object_transforms: vec![PrimitiveTransform::identity()],
        }
    }

    fn alloc_object_id(&mut self) -> u32 {
        let id = self.next_object_id;
        self.next_object_id = self.next_object_id.saturating_add(1);
        id
    }

    fn register_object_transform(&mut self, object_id: u32, transform: PrimitiveTransform) {
        let needed = object_id as usize + 1;
        if self.object_transforms.len() < needed {
            self.object_transforms
                .resize(needed, PrimitiveTransform::identity());
        }
        self.object_transforms[object_id as usize] = transform;
    }

    fn intern_material(&mut self, mat: MaterialKindRt) -> u32 {
        if let Some((idx, _)) = self
            .materials
            .iter()
            .enumerate()
            .find(|(_, existing)| **existing == mat)
        {
            idx as u32
        } else {
            self.materials.push(mat);
            (self.materials.len() - 1) as u32
        }
    }
}

#[derive(Clone, Copy)]
enum PathLight {
    Point {
        position: ApiVec3,
        intensity: Spectrum,
    },
    Env {
        radiance: Spectrum,
    },
}

trait Accelerator {
    fn from_scene(scene: CompiledScene) -> Self
    where
        Self: Sized;

    fn distance_info(&self, p: Vec3) -> DistanceInfo;

    fn distance(&self, p: Vec3) -> f32 {
        self.distance_info(p).distance
    }
}

struct NaiveAccel {
    scene: CompiledScene,
}

impl Accelerator for NaiveAccel {
    fn from_scene(scene: CompiledScene) -> Self {
        Self { scene }
    }

    fn distance_info(&self, p: Vec3) -> DistanceInfo {
        sdf_distance_info(&self.scene.root, p)
    }
}

struct BvhAccel {
    scene: CompiledScene,
}

impl Accelerator for BvhAccel {
    fn from_scene(scene: CompiledScene) -> Self {
        Self { scene }
    }

    fn distance_info(&self, p: Vec3) -> DistanceInfo {
        let dist_bound = distance_to_aabb(p, self.scene.bounds);
        let mut info = sdf_distance_info(&self.scene.root, p);
        info.distance = info.distance.max(dist_bound);
        info
    }
}

struct BricksAccel {
    scene: CompiledScene,
}

impl Accelerator for BricksAccel {
    fn from_scene(scene: CompiledScene) -> Self {
        Self { scene }
    }

    fn distance_info(&self, p: Vec3) -> DistanceInfo {
        let dist_bound = distance_to_aabb(p, self.scene.bounds);
        let mut info = sdf_distance_info(&self.scene.root, p);
        info.distance = info.distance.max(dist_bound * 0.75);
        info
    }
}

pub fn render_depth_png(
    state: &EvalState,
    output_path: &Path,
    options: RenderOptions,
) -> Result<(), RenderError> {
    render_depth_png_with_accel(state, output_path, options, AccelMode::Naive)
}

pub fn render_depth_png_with_accel(
    state: &EvalState,
    output_path: &Path,
    options: RenderOptions,
    accel_mode: AccelMode,
) -> Result<(), RenderError> {
    let root = find_scene_root(state).ok_or(RenderError::MissingSceneRoot)?;
    let default_material = parse_material(state, root);
    let scene = compile_scene(state, root, default_material)?;
    let setup = build_render_setup(state, &scene, options);
    let image = match accel_mode {
        AccelMode::Naive => render_with_accel::<NaiveAccel>(scene, setup, options),
        AccelMode::Bvh => render_with_accel::<BvhAccel>(scene, setup, options),
        AccelMode::Bricks => render_with_accel::<BricksAccel>(scene, setup, options),
    };
    image.save(output_path)?;
    Ok(())
}

pub fn render_pathtrace_png_with_accel(
    state: &EvalState,
    output_path: &Path,
    options: RenderOptions,
    accel_mode: AccelMode,
    spp: u32,
    max_bounces: u32,
) -> Result<(), RenderError> {
    let settings = PathtraceSettings {
        spp,
        max_bounces,
        preview_every: spp.max(1),
        min_spp: spp.max(1),
        noise_threshold: 0.0,
    };
    let image =
        render_pathtrace_progressive_with_accel(state, options, accel_mode, settings, |_, _| {
            Ok(())
        })?;
    image.save(output_path)?;
    Ok(())
}

pub fn render_ray_png_with_accel(
    state: &EvalState,
    output_path: &Path,
    options: RenderOptions,
    accel_mode: AccelMode,
    max_depth: u32,
) -> Result<(), RenderError> {
    let settings = RaySettings {
        max_depth: max_depth.max(1),
        tile_size: options.width.max(options.height),
        debug_aov: None,
    };
    let image =
        render_ray_progressive_with_accel(state, options, accel_mode, settings, |_, _| Ok(()))?;
    image.save(output_path)?;
    Ok(())
}

pub fn render_ray_progressive_with_accel(
    state: &EvalState,
    options: RenderOptions,
    accel_mode: AccelMode,
    settings: RaySettings,
    mut on_tile: impl FnMut(RayProgress, &RgbImage) -> Result<(), RenderError>,
) -> Result<RgbImage, RenderError> {
    let root = find_scene_root(state).ok_or(RenderError::MissingSceneRoot)?;
    let default_material = parse_material(state, root);
    let scene = compile_scene(state, root, default_material)?;
    let setup = build_render_setup(state, &scene, options);
    let image = match accel_mode {
        AccelMode::Naive => render_ray_with_accel_progressive::<NaiveAccel>(
            scene,
            setup,
            options,
            settings,
            &mut on_tile,
        )?,
        AccelMode::Bvh => render_ray_with_accel_progressive::<BvhAccel>(
            scene,
            setup,
            options,
            settings,
            &mut on_tile,
        )?,
        AccelMode::Bricks => render_ray_with_accel_progressive::<BricksAccel>(
            scene,
            setup,
            options,
            settings,
            &mut on_tile,
        )?,
    };
    Ok(image)
}

pub fn render_preview_progressive_with_accel(
    state: &EvalState,
    options: RenderOptions,
    accel_mode: AccelMode,
    tile_size: u32,
    mut on_tile: impl FnMut(PreviewProgress, &RgbImage) -> Result<(), RenderError>,
) -> Result<RgbImage, RenderError> {
    let root = find_scene_root(state).ok_or(RenderError::MissingSceneRoot)?;
    let default_material = parse_material(state, root);
    let scene = compile_scene(state, root, default_material)?;
    let setup = build_render_setup(state, &scene, options);
    let image = match accel_mode {
        AccelMode::Naive => render_preview_with_accel_progressive::<NaiveAccel>(
            scene,
            setup,
            options,
            tile_size,
            &mut on_tile,
        )?,
        AccelMode::Bvh => render_preview_with_accel_progressive::<BvhAccel>(
            scene,
            setup,
            options,
            tile_size,
            &mut on_tile,
        )?,
        AccelMode::Bricks => render_preview_with_accel_progressive::<BricksAccel>(
            scene,
            setup,
            options,
            tile_size,
            &mut on_tile,
        )?,
    };
    Ok(image)
}

pub fn render_pathtrace_progressive_with_accel(
    state: &EvalState,
    options: RenderOptions,
    accel_mode: AccelMode,
    settings: PathtraceSettings,
    mut on_preview: impl FnMut(PathtraceProgress, &RgbImage) -> Result<(), RenderError>,
) -> Result<RgbImage, RenderError> {
    let root = find_scene_root(state).ok_or(RenderError::MissingSceneRoot)?;
    let default_material = parse_material(state, root);
    let scene = compile_scene(state, root, default_material)?;
    let setup = build_render_setup(state, &scene, options);
    match accel_mode {
        AccelMode::Naive => render_pathtrace_with_accel_progressive::<NaiveAccel>(
            scene,
            setup,
            options,
            settings,
            &mut on_preview,
        ),
        AccelMode::Bvh => render_pathtrace_with_accel_progressive::<BvhAccel>(
            scene,
            setup,
            options,
            settings,
            &mut on_preview,
        ),
        AccelMode::Bricks => render_pathtrace_with_accel_progressive::<BricksAccel>(
            scene,
            setup,
            options,
            settings,
            &mut on_preview,
        ),
    }
}

pub fn extract_scene_render_settings(state: &EvalState) -> SceneRenderSettings {
    let mut out = SceneRenderSettings::default();
    let settings_obj = ["render", "render_settings", "settings"]
        .into_iter()
        .filter_map(|key| state.bindings.get(key))
        .find_map(|binding| match &binding.value {
            Value::Object(obj) => Some(obj),
            _ => None,
        });

    let Some(obj) = settings_obj else {
        return out;
    };

    out.width = read_number_field(obj, &["width"]).and_then(float_to_u32);
    out.height = read_number_field(obj, &["height"]).and_then(float_to_u32);
    out.max_steps = read_number_field(obj, &["max_steps"]).and_then(float_to_u32);
    out.max_dist = read_number_field(obj, &["max_dist"]);
    out.epsilon = read_number_field(obj, &["epsilon"]);
    out.camera_z = read_number_field(obj, &["camera_z"]);
    out.fov_y_degrees = read_number_field(obj, &["fov_y", "fov"]);
    out.accel = read_accel_field(obj, "accel");
    out.trace_spp = read_number_field(obj, &["spp", "samples", "trace_spp"]).and_then(float_to_u32);
    out.trace_bounces =
        read_number_field(obj, &["bounces", "max_bounces", "trace_bounces", "depth"])
            .and_then(float_to_u32);
    out.trace_min_spp =
        read_number_field(obj, &["min_spp", "trace_min_spp"]).and_then(float_to_u32);
    out.trace_noise_threshold = read_number_field(
        obj,
        &[
            "noise_threshold",
            "trace_noise_threshold",
            "adaptive_threshold",
        ],
    );
    out
}

fn find_scene_root(state: &EvalState) -> Option<&Value> {
    for key in ["scene", "result", "b", "s"] {
        if let Some(binding) = state.bindings.get(key) {
            return Some(&binding.value);
        }
    }
    None
}

fn compile_scene(
    state: &EvalState,
    value: &Value,
    default_material: MaterialKindRt,
) -> Result<CompiledScene, RenderError> {
    let mut ctx = CompileContext::new(default_material, state);
    let shared_state = Arc::new(state.clone());
    let root = compile_sdf(&shared_state, value, &mut ctx)?;
    let center = sdf_center(&root);
    let bounds = sdf_bounds(&root);
    Ok(CompiledScene {
        root,
        center,
        bounds,
        materials: ctx.materials,
        object_transforms: ctx.object_transforms,
    })
}

fn compile_sdf(
    state: &Arc<EvalState>,
    value: &Value,
    ctx: &mut CompileContext,
) -> Result<SdfNode, RenderError> {
    let Value::Object(object) = value else {
        return Err(RenderError::ExpectedObject);
    };
    let type_name = object.type_name.as_deref().unwrap_or("anonymous");

    match type_name {
        "Sphere" => {
            let transform = read_transform(object);
            let radius = read_number_field(object, &["radius", "r"]).unwrap_or(1.0_f32);
            let object_id = ctx.alloc_object_id();
            ctx.register_object_transform(object_id, transform);
            let material_id = primitive_material_id(state, object, ctx);
            Ok(SdfNode::Sphere {
                transform,
                radius,
                object_id,
                material_id,
            })
        }
        "Box" => {
            let transform = read_transform(object);
            let size = read_vec3_field(object, "size").unwrap_or(Vec3::new(1.0, 1.0, 1.0));
            let object_id = ctx.alloc_object_id();
            ctx.register_object_transform(object_id, transform);
            let material_id = primitive_material_id(state, object, ctx);
            Ok(SdfNode::Box {
                transform,
                half_size: size.mul(0.5),
                object_id,
                material_id,
            })
        }
        "Cylinder" => {
            let transform = read_transform(object);
            let radius = read_number_field(object, &["radius", "r"]).unwrap_or(1.0);
            let height = read_number_field(object, &["height", "h"]).unwrap_or(1.0);
            let object_id = ctx.alloc_object_id();
            ctx.register_object_transform(object_id, transform);
            let material_id = primitive_material_id(state, object, ctx);
            Ok(SdfNode::Cylinder {
                transform,
                radius,
                half_height: height * 0.5,
                object_id,
                material_id,
            })
        }
        "Torus" => {
            let transform = read_transform(object);
            let major_radius = read_number_field(object, &["major_radius", "R"]).unwrap_or(1.0);
            let minor_radius = read_number_field(object, &["minor_radius", "r"]).unwrap_or(0.25);
            let object_id = ctx.alloc_object_id();
            ctx.register_object_transform(object_id, transform);
            let material_id = primitive_material_id(state, object, ctx);
            Ok(SdfNode::Torus {
                transform,
                major_radius,
                minor_radius,
                object_id,
                material_id,
            })
        }
        custom if state.sdf_defs.contains_key(custom) => {
            let transform = read_transform(object);
            let object_id = ctx.alloc_object_id();
            ctx.register_object_transform(object_id, transform);
            let material_id = primitive_material_id(state, object, ctx);
            Ok(SdfNode::Custom {
                transform,
                runtime: Arc::new(CustomSdfRuntime {
                    state: Arc::clone(state),
                    name: custom.to_string(),
                }),
                bounds_radius: eval_custom_sdf_bounds_radius(state, custom),
                object_id,
                material_id,
            })
        }
        "add" => {
            let lhs = compile_sdf(state, required_field(object, "lhs")?, ctx)?;
            let rhs = compile_sdf(state, required_field(object, "rhs")?, ctx)?;
            Ok(SdfNode::Union {
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            })
        }
        "sub" => {
            let lhs = compile_sdf(state, required_field(object, "lhs")?, ctx)?;
            let rhs = compile_sdf(state, required_field(object, "rhs")?, ctx)?;
            Ok(SdfNode::Subtract {
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            })
        }
        "smooth" => {
            let base = compile_sdf(state, required_field(object, "base")?, ctx)?;
            let k = match required_field(object, "k")? {
                Value::Number(v) => *v as f32,
                _ => 0.0,
            };
            Ok(SdfNode::Smooth {
                base: Box::new(base),
                k,
            })
        }
        "round" | "bevel" | "chamfer" => {
            let base = compile_sdf(state, required_field(object, "base")?, ctx)?;
            let r = match required_field(object, "r")? {
                Value::Number(v) => *v as f32,
                _ => 0.0,
            };
            Ok(SdfNode::Round {
                base: Box::new(base),
                r,
            })
        }
        other => Err(RenderError::UnsupportedObjectType(other.to_string())),
    }
}

fn required_field<'a>(obj: &'a ObjectValue, name: &str) -> Result<&'a Value, RenderError> {
    obj.fields.get(name).ok_or_else(|| {
        RenderError::UnsupportedObjectType(obj.type_name.clone().unwrap_or_default())
    })
}

fn primitive_material_id(
    state: &Arc<EvalState>,
    obj: &ObjectValue,
    ctx: &mut CompileContext,
) -> u32 {
    if let Some(Value::Object(mat_obj)) = obj.fields.get("material") {
        return ctx.intern_material(material_from_object(state, mat_obj));
    }
    ctx.intern_material(ctx.default_material)
}

fn read_number_field(obj: &ObjectValue, names: &[&str]) -> Option<f32> {
    names.iter().find_map(|name| match obj.fields.get(*name) {
        Some(Value::Number(value)) => Some(*value as f32),
        _ => None,
    })
}

fn read_number_path(obj: &ObjectValue, path: &[&str]) -> Option<f32> {
    let (last, parents) = path.split_last()?;
    let mut current = obj;
    for segment in parents {
        let Value::Object(next) = current.fields.get(*segment)? else {
            return None;
        };
        current = next;
    }
    read_number_field(current, &[*last])
}

fn float_to_u32(v: f32) -> Option<u32> {
    if v.is_finite() && v >= 1.0 {
        let rounded = v.round();
        if (rounded - v).abs() < 1.0e-5 {
            return Some(rounded as u32);
        }
    }
    None
}

fn read_accel_field(obj: &ObjectValue, name: &str) -> Option<AccelMode> {
    let value = obj.fields.get(name)?;
    match value {
        Value::Object(v) => match v.type_name.as_deref() {
            Some("Naive") | Some("naive") => Some(AccelMode::Naive),
            Some("Bvh") | Some("bvh") => Some(AccelMode::Bvh),
            Some("Bricks") | Some("bricks") => Some(AccelMode::Bricks),
            _ => None,
        },
        Value::Number(n) => match *n as i32 {
            0 => Some(AccelMode::Naive),
            1 => Some(AccelMode::Bvh),
            2 => Some(AccelMode::Bricks),
            _ => None,
        },
    }
}

fn read_center(obj: &ObjectValue) -> Vec3 {
    Vec3::new(
        read_number_path(obj, &["pos", "x"])
            .or_else(|| read_number_field(obj, &["x"]))
            .unwrap_or(0.0),
        read_number_path(obj, &["pos", "y"])
            .or_else(|| read_number_field(obj, &["y"]))
            .unwrap_or(0.0),
        read_number_path(obj, &["pos", "z"])
            .or_else(|| read_number_field(obj, &["z"]))
            .unwrap_or(0.0),
    )
}

fn read_transform(obj: &ObjectValue) -> PrimitiveTransform {
    PrimitiveTransform {
        center: read_center(obj),
        rot_deg: Vec3::new(
            read_number_path(obj, &["rot", "x"])
                .or_else(|| read_number_field(obj, &["rot_x"]))
                .unwrap_or(0.0),
            read_number_path(obj, &["rot", "y"])
                .or_else(|| read_number_field(obj, &["rot_y"]))
                .unwrap_or(0.0),
            read_number_path(obj, &["rot", "z"])
                .or_else(|| read_number_field(obj, &["rot_z"]))
                .unwrap_or(0.0),
        ),
    }
}

fn read_vec3_field(obj: &ObjectValue, name: &str) -> Option<Vec3> {
    let value = obj.fields.get(name)?;
    let Value::Object(vec_obj) = value else {
        return None;
    };
    let x = read_number_field(vec_obj, &["x"])?;
    let y = read_number_field(vec_obj, &["y"])?;
    let z = read_number_field(vec_obj, &["z"])?;
    Some(Vec3::new(x, y, z))
}

fn render_with_accel<A: Accelerator + Sync>(
    scene: CompiledScene,
    setup: RenderSetup,
    options: RenderOptions,
) -> RgbImage {
    let accel = A::from_scene(scene);
    render_preview_image(&accel, &setup, options, options.width.max(options.height))
}

fn render_ray_with_accel_progressive<A: Accelerator + Sync>(
    scene: CompiledScene,
    setup: RenderSetup,
    options: RenderOptions,
    settings: RaySettings,
    on_tile: &mut impl FnMut(RayProgress, &RgbImage) -> Result<(), RenderError>,
) -> Result<RgbImage, RenderError> {
    let accel = A::from_scene(scene);
    let aspect = options.width as f32 / options.height as f32;
    let width = options.width as usize;
    let height = options.height as usize;
    let mut buffer = vec![0_u8; width * height * 3];
    let ray_ctx = RayTraceCtx {
        options,
        max_depth: settings.max_depth.max(1),
    };
    let tile_size = settings.tile_size.max(1) as usize;
    let tiles_x = width.div_ceil(tile_size);
    let tiles_y = height.div_ceil(tile_size);
    let tiles_total = (tiles_x * tiles_y) as u32;
    let start = Instant::now();
    let mut tiles_done = 0_u32;

    for ty in (0..height).step_by(tile_size) {
        for tx in (0..width).step_by(tile_size) {
            let tile_w = (width - tx).min(tile_size);
            let tile_h = (height - ty).min(tile_size);
            let mut tile = vec![0_u8; tile_w * tile_h * 3];
            tile.par_chunks_mut(tile_w * 3)
                .enumerate()
                .for_each(|(ly, row)| {
                    let y = ty + ly;
                    let y_u32 = y as u32;
                    for lx in 0..tile_w {
                        let x = tx + lx;
                        let x_u32 = x as u32;
                        let px = ((x_u32 as f32 + 0.5) / options.width as f32) * 2.0 - 1.0;
                        let py = 1.0 - ((y_u32 as f32 + 0.5) / options.height as f32) * 2.0;
                        let ray = setup.camera.generate_ray(px * aspect, py);
                        let origin = from_api_vec3(ray.origin);
                        let dir = from_api_vec3(ray.direction).normalize();
                        let rgb = if let Some(aov) = settings.debug_aov {
                            let c =
                                ray::trace_ray_debug_aov(&accel, &setup, ray_ctx, origin, dir, aov);
                            spectrum_to_rgb8(c)
                        } else {
                            let c = ray::trace_ray_recursive(
                                &accel,
                                &setup,
                                ray_ctx,
                                origin,
                                dir,
                                MediumState::air(),
                                0,
                            );
                            spectrum_to_rgb8_reinhard(c)
                        };
                        let i = lx * 3;
                        row[i] = rgb[0];
                        row[i + 1] = rgb[1];
                        row[i + 2] = rgb[2];
                    }
                });
            for ly in 0..tile_h {
                let dst = ((ty + ly) * width + tx) * 3;
                let src = (ly * tile_w) * 3;
                let len = tile_w * 3;
                buffer[dst..dst + len].copy_from_slice(&tile[src..src + len]);
            }
            tiles_done += 1;
            let image = RgbImage::from_vec(options.width, options.height, buffer.clone())
                .expect("pixel buffer length must match image dimensions");
            on_tile(
                RayProgress {
                    tiles_done,
                    tiles_total,
                    elapsed_ms: start.elapsed().as_millis(),
                },
                &image,
            )?;
        }
    }

    Ok(RgbImage::from_vec(options.width, options.height, buffer)
        .expect("pixel buffer length must match image dimensions"))
}

fn render_preview_image(
    accel: &(impl Accelerator + Sync),
    setup: &RenderSetup,
    options: RenderOptions,
    tile_size: u32,
) -> RgbImage {
    render_preview_tiled(accel, setup, options, tile_size, &mut |_, _| Ok(()))
        .expect("preview rendering without callback failure should succeed")
}

fn render_preview_with_accel_progressive<A: Accelerator + Sync>(
    scene: CompiledScene,
    setup: RenderSetup,
    options: RenderOptions,
    tile_size: u32,
    on_tile: &mut impl FnMut(PreviewProgress, &RgbImage) -> Result<(), RenderError>,
) -> Result<RgbImage, RenderError> {
    let accel = A::from_scene(scene);
    render_preview_tiled(&accel, &setup, options, tile_size, on_tile)
}

fn render_preview_tiled(
    accel: &(impl Accelerator + Sync),
    setup: &RenderSetup,
    options: RenderOptions,
    tile_size: u32,
    on_tile: &mut impl FnMut(PreviewProgress, &RgbImage) -> Result<(), RenderError>,
) -> Result<RgbImage, RenderError> {
    let aspect = options.width as f32 / options.height as f32;
    let width = options.width as usize;
    let height = options.height as usize;
    let mut buffer = vec![0_u8; width * height * 3];
    let tile_size = tile_size.max(1) as usize;
    let tiles_x = width.div_ceil(tile_size);
    let tiles_y = height.div_ceil(tile_size);
    let tiles_total = (tiles_x * tiles_y) as u32;
    let start = Instant::now();
    let mut tiles_done = 0_u32;

    for ty in (0..height).step_by(tile_size) {
        for tx in (0..width).step_by(tile_size) {
            let tile_w = (width - tx).min(tile_size);
            let tile_h = (height - ty).min(tile_size);
            let mut tile = vec![0_u8; tile_w * tile_h * 3];
            tile.par_chunks_mut(tile_w * 3)
                .enumerate()
                .for_each(|(ly, row)| {
                    let y = ty + ly;
                    let y_u32 = y as u32;
                    for lx in 0..tile_w {
                        let x = tx + lx;
                        let x_u32 = x as u32;
                        let px = ((x_u32 as f32 + 0.5) / options.width as f32) * 2.0 - 1.0;
                        let py = 1.0 - ((y_u32 as f32 + 0.5) / options.height as f32) * 2.0;
                        let ray = setup.camera.generate_ray(px * aspect, py);
                        let origin = from_api_vec3(ray.origin);
                        let dir = from_api_vec3(ray.direction).normalize();
                        let hit = raymarch_hit(accel, origin, dir, options, 0.0, options.max_dist);
                        let rgb = match hit {
                            Some(hit) => {
                                let material =
                                    resolve_material_at_hit(setup, hit, dir.mul(-1.0).normalize());
                                let bsdf_ctx = build_bsdf_context(setup, hit, dir.mul(-1.0), 1.0);
                                let lit =
                                    shade_preview_color(setup, &setup.lights, material, bsdf_ctx);
                                spectrum_to_rgb8_reinhard(lit)
                            }
                            None => [0, 0, 0],
                        };
                        let i = lx * 3;
                        row[i] = rgb[0];
                        row[i + 1] = rgb[1];
                        row[i + 2] = rgb[2];
                    }
                });
            for ly in 0..tile_h {
                let dst = ((ty + ly) * width + tx) * 3;
                let src = (ly * tile_w) * 3;
                let len = tile_w * 3;
                buffer[dst..dst + len].copy_from_slice(&tile[src..src + len]);
            }
            tiles_done += 1;
            let image = RgbImage::from_vec(options.width, options.height, buffer.clone())
                .expect("pixel buffer length must match image dimensions");
            on_tile(
                PreviewProgress {
                    tiles_done,
                    tiles_total,
                    elapsed_ms: start.elapsed().as_millis(),
                },
                &image,
            )?;
        }
    }

    Ok(RgbImage::from_vec(options.width, options.height, buffer)
        .expect("pixel buffer length must match image dimensions"))
}

fn render_pathtrace_with_accel_progressive<A: Accelerator + Sync>(
    scene: CompiledScene,
    setup: RenderSetup,
    options: RenderOptions,
    settings: PathtraceSettings,
    on_preview: &mut impl FnMut(PathtraceProgress, &RgbImage) -> Result<(), RenderError>,
) -> Result<RgbImage, RenderError> {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let accel = A::from_scene(scene);
    let aspect = options.width as f32 / options.height as f32;
    let width_usize = options.width as usize;
    let spp = settings.spp.max(1);
    let max_bounces = settings.max_bounces.max(1);
    let preview_every = settings.preview_every.max(1);
    let min_spp = settings.min_spp.max(1).min(spp);
    let noise_threshold = settings.noise_threshold.max(0.0);
    let mut pixels = vec![PixelAccumulator::new(); width_usize * options.height as usize];
    let start = Instant::now();

    let mut samples_done = 0_u32;
    let mut active_pixels = pixels.len() as u32;

    while samples_done < spp && active_pixels > 0 {
        let target = (samples_done + preview_every).min(spp);
        while samples_done < target && active_pixels > 0 {
            let sample_idx = samples_done;
            let active_after = AtomicUsize::new(active_pixels as usize);
            pixels.par_iter_mut().enumerate().for_each(|(idx, pixel)| {
                if !pixel.active {
                    return;
                }
                let x_u32 = (idx % width_usize) as u32;
                let y_u32 = (idx / width_usize) as u32;
                let mut rng =
                    XorShift64::new(seed_pixel_sample(x_u32, y_u32, options.width, sample_idx));
                let jx = rng.next_f32() - 0.5;
                let jy = rng.next_f32() - 0.5;
                let px = (((x_u32 as f32 + 0.5 + jx) / options.width as f32) * 2.0 - 1.0) * aspect;
                let py = 1.0 - ((y_u32 as f32 + 0.5 + jy) / options.height as f32) * 2.0;
                let ray = setup.camera.generate_ray(px, py);
                let origin = from_api_vec3(ray.origin);
                let dir = from_api_vec3(ray.direction).normalize();
                let sample =
                    path::trace_path(&accel, &setup, origin, dir, options, max_bounces, &mut rng);
                pixel.add_sample(sample);
                if noise_threshold > 0.0
                    && pixel.count >= min_spp
                    && pixel.relative_error() <= noise_threshold
                {
                    pixel.active = false;
                    active_after.fetch_sub(1, Ordering::Relaxed);
                }
            });
            samples_done += 1;
            active_pixels = active_after.load(Ordering::Relaxed) as u32;
        }

        let image = image_from_pixels(&pixels, options.width, options.height);
        on_preview(
            PathtraceProgress {
                samples_done,
                samples_total: spp,
                active_pixels,
                elapsed_ms: start.elapsed().as_millis(),
            },
            &image,
        )?;
    }

    Ok(image_from_pixels(&pixels, options.width, options.height))
}

fn sdf_center(node: &SdfNode) -> Vec3 {
    match node {
        SdfNode::Sphere { transform, .. } => transform.center,
        SdfNode::Box { transform, .. } => transform.center,
        SdfNode::Cylinder { transform, .. } => transform.center,
        SdfNode::Torus { transform, .. } => transform.center,
        SdfNode::Custom { transform, .. } => transform.center,
        SdfNode::Union { lhs, rhs } => {
            let l = sdf_center(lhs);
            let r = sdf_center(rhs);
            Vec3::new((l.x + r.x) * 0.5, (l.y + r.y) * 0.5, (l.z + r.z) * 0.5)
        }
        SdfNode::Subtract { lhs, .. } => sdf_center(lhs),
        SdfNode::Smooth { base, .. } => sdf_center(base),
        SdfNode::Round { base, .. } => sdf_center(base),
    }
}

fn raymarch_hit(
    accel: &(impl Accelerator + Sync),
    origin: Vec3,
    dir: Vec3,
    options: RenderOptions,
    min_t: f32,
    max_t: f32,
) -> Option<RayHit> {
    let mut traveled = min_t.max(0.0);
    let mut previous_traveled = traveled;
    for _ in 0..options.max_steps {
        if traveled > max_t {
            return None;
        }
        let p = origin.add(dir.mul(traveled));
        let info = accel.distance_info(p);
        let d = info.distance;
        if d.abs() < options.epsilon {
            let refined = refine_hit_distance(
                accel,
                origin,
                dir,
                previous_traveled,
                traveled,
                options.epsilon,
                10,
            );
            let position = origin.add(dir.mul(refined));
            let normal = estimate_normal(accel, position, (options.epsilon * 4.0).max(1.0e-5));
            let front_face = normal.dot(dir) < 0.0;
            let final_info = accel.distance_info(position);
            return Some(RayHit {
                t: refined,
                position,
                normal,
                front_face,
                object_id: final_info.object_id,
                material_id: final_info.material_id,
            });
        }
        previous_traveled = traveled;
        traveled += d.abs().max((options.epsilon * 0.5).max(1.0e-5));
    }
    None
}

fn refine_hit_distance(
    accel: &(impl Accelerator + Sync),
    origin: Vec3,
    dir: Vec3,
    mut lo: f32,
    mut hi: f32,
    epsilon: f32,
    iterations: u32,
) -> f32 {
    for _ in 0..iterations {
        let mid = 0.5 * (lo + hi);
        let p = origin.add(dir.mul(mid));
        let d = accel.distance(p).abs();
        if d < epsilon {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    hi
}

fn estimate_normal(accel: &(impl Accelerator + Sync), p: Vec3, e: f32) -> Vec3 {
    let dx =
        accel.distance(Vec3::new(p.x + e, p.y, p.z)) - accel.distance(Vec3::new(p.x - e, p.y, p.z));
    let dy =
        accel.distance(Vec3::new(p.x, p.y + e, p.z)) - accel.distance(Vec3::new(p.x, p.y - e, p.z));
    let dz =
        accel.distance(Vec3::new(p.x, p.y, p.z + e)) - accel.distance(Vec3::new(p.x, p.y, p.z - e));
    Vec3::new(dx, dy, dz).normalize()
}

fn shade_color(
    accel: &(impl Accelerator + Sync),
    setup: &RenderSetup,
    options: RenderOptions,
    lights: &[Box<dyn Light>],
    material: MaterialKindRt,
    bsdf_ctx: BsdfContextBase,
) -> Spectrum {
    if lights.is_empty() {
        return Spectrum::rgb(1.0, 1.0, 1.0);
    }

    let p = to_api_vec3(bsdf_ctx.hit.position);
    let n = bsdf_ctx.normal.normalize();
    let mut color = Spectrum::black();
    for light in lights {
        let sample = light.sample_li(p);
        let wi = sample.wi.normalize();
        let ndotl = n.x * wi.x + n.y * wi.y + n.z * wi.z;
        if ndotl <= 0.0 {
            continue;
        }
        let shadow_origin = bsdf_ctx
            .hit
            .position
            .add(n.mul((options.epsilon * 12.0).max(1.0e-4)));
        let max_t = if sample.distance.is_finite() {
            (sample.distance - (options.epsilon * 8.0)).max(0.0)
        } else {
            options.max_dist
        };
        if max_t <= 0.0 {
            continue;
        }
        let shadow = shadow_visibility(
            accel,
            shadow_origin,
            from_api_vec3(wi),
            max_t,
            options.epsilon,
        );
        if shadow <= 0.0 {
            continue;
        }
        let f = eval_bsdf(setup, material, bsdf_ctx, from_api_vec3(wi));
        color = color + (f * sample.radiance).scale(ndotl * shadow);
    }
    color
}

fn shade_preview_color(
    setup: &RenderSetup,
    lights: &[Box<dyn Light>],
    material: MaterialKindRt,
    bsdf_ctx: BsdfContextBase,
) -> Spectrum {
    if lights.is_empty() {
        return Spectrum::rgb(1.0, 1.0, 1.0);
    }

    let p = to_api_vec3(bsdf_ctx.hit.position);
    let n = bsdf_ctx.normal.normalize();
    let mut color = Spectrum::black();
    for light in lights {
        let sample = light.sample_li(p);
        let wi = sample.wi.normalize();
        let ndotl = n.x * wi.x + n.y * wi.y + n.z * wi.z;
        if ndotl <= 0.0 {
            continue;
        }
        let f = eval_bsdf(setup, material, bsdf_ctx, from_api_vec3(wi));
        color = color + (f * sample.radiance).scale(ndotl);
    }
    color
}

fn material_id_color(material_id: u32) -> Spectrum {
    let mut x = material_id
        .wrapping_mul(0x9E37_79B9)
        .wrapping_add(0x7F4A_7C15);
    x ^= x >> 16;
    x = x.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 13;
    let r = (x & 0xFF) as f32 / 255.0;
    let g = ((x >> 8) & 0xFF) as f32 / 255.0;
    let b = ((x >> 16) & 0xFF) as f32 / 255.0;
    Spectrum::rgb(r, g, b)
}

fn env_radiance(lights: &[PathLight]) -> Spectrum {
    let mut sum = Spectrum::black();
    for light in lights {
        if let PathLight::Env { radiance } = light {
            sum = sum + *radiance;
        }
    }
    sum
}

fn env_light_pdf_for_dir(
    lights: &[PathLight],
    setup: &RenderSetup,
    mat: MaterialKindRt,
    bsdf_ctx: BsdfContextBase,
    wi: Vec3,
) -> f32 {
    if lights.is_empty() {
        return 0.0;
    }
    let env_count = lights
        .iter()
        .filter(|light| matches!(light, PathLight::Env { .. }))
        .count() as f32;
    if env_count <= 0.0 {
        return 0.0;
    }
    let select_pdf = env_count / lights.len() as f32;
    select_pdf * pdf_bsdf(setup, mat, bsdf_ctx, wi).max(1.0e-6)
}

struct DirectLightingCtx<'a, A: Accelerator + Sync> {
    accel: &'a A,
    setup: &'a RenderSetup,
    options: RenderOptions,
    lights: &'a [PathLight],
}

#[derive(Clone, Copy)]
struct RayTraceCtx {
    options: RenderOptions,
    max_depth: u32,
}

#[derive(Clone, Copy)]
struct MediumState {
    ior: f32,
    absorption_color: Spectrum,
    density: f32,
}

impl MediumState {
    const fn air() -> Self {
        Self {
            ior: 1.0,
            absorption_color: Spectrum::rgb(1.0, 1.0, 1.0),
            density: 0.0,
        }
    }
}

#[derive(Clone, Copy)]
struct BsdfContextBase {
    hit: RayHit,
    local_position: Vec3,
    normal: Vec3,
    wo: Vec3,
    current_ior: f32,
}

fn build_bsdf_context(
    setup: &RenderSetup,
    hit: RayHit,
    view_dir: Vec3,
    current_ior: f32,
) -> BsdfContextBase {
    let transform = setup
        .object_transforms
        .get(hit.object_id as usize)
        .copied()
        .unwrap_or_else(PrimitiveTransform::identity);
    let local_position = to_local(hit.position, transform);
    let geometric_normal = if hit.front_face {
        hit.normal.normalize()
    } else {
        hit.normal.mul(-1.0).normalize()
    };
    let material = material_for_id(&setup.materials, hit.material_id);
    let normal = resolve_dynamic_normal(
        &setup.state,
        &setup.material_def_names,
        material,
        hit,
        local_position,
        view_dir,
        geometric_normal,
    )
    .unwrap_or(geometric_normal);

    BsdfContextBase {
        hit,
        local_position,
        normal,
        wo: view_dir.normalize(),
        current_ior,
    }
}

fn estimate_direct_mis<A: Accelerator + Sync>(
    ctx: &DirectLightingCtx<'_, A>,
    mat: MaterialKindRt,
    hit_point: Vec3,
    bsdf_ctx: BsdfContextBase,
    rng: &mut XorShift64,
) -> Spectrum {
    if ctx.lights.is_empty() {
        return Spectrum::black();
    }

    let li = sample_one_light(
        ctx.lights,
        mat,
        hit_point,
        bsdf_ctx.normal,
        bsdf_ctx.wo,
        rng,
    );
    if li.pdf <= 1.0e-6 {
        return Spectrum::black();
    }
    let cos_theta = bsdf_ctx.normal.dot(li.wi).max(0.0);
    if cos_theta <= 0.0 {
        return Spectrum::black();
    }

    let shadow_origin = hit_point.add(
        bsdf_ctx
            .normal
            .mul((ctx.options.epsilon * 12.0).max(1.0e-4)),
    );
    let vis = shadow_visibility(
        ctx.accel,
        shadow_origin,
        li.wi,
        li.max_t,
        ctx.options.epsilon,
    );
    if vis <= 0.0 {
        return Spectrum::black();
    }

    let f = eval_bsdf(ctx.setup, mat, bsdf_ctx, li.wi);
    let bsdf_pdf = pdf_bsdf(ctx.setup, mat, bsdf_ctx, li.wi).max(1.0e-6);
    let w = if li.delta {
        1.0
    } else {
        power_heuristic(li.pdf, bsdf_pdf)
    };
    (f * li.radiance).scale((cos_theta * vis * w) / li.pdf.max(1.0e-6))
}

#[derive(Clone, Copy)]
struct DirectLightSample {
    wi: Vec3,
    radiance: Spectrum,
    pdf: f32,
    max_t: f32,
    delta: bool,
}

fn sample_one_light(
    lights: &[PathLight],
    _mat: MaterialKindRt,
    hit_point: Vec3,
    normal: Vec3,
    _wo: Vec3,
    rng: &mut XorShift64,
) -> DirectLightSample {
    let count = lights.len() as f32;
    let idx = ((rng.next_f32() * count).floor() as usize).min(lights.len() - 1);
    let select_pdf = 1.0 / count;
    match lights[idx] {
        PathLight::Point {
            position,
            intensity,
        } => {
            let to_light = from_api_vec3(position).sub(hit_point);
            let dist = to_light.length().max(1.0e-4);
            let wi = to_light.mul(1.0 / dist);
            let att = 1.0 / (dist * dist);
            DirectLightSample {
                wi,
                radiance: intensity.scale(att),
                pdf: select_pdf,
                max_t: dist - 1.0e-4,
                delta: true,
            }
        }
        PathLight::Env { radiance } => {
            let wi = cosine_sample_hemisphere(normal, rng);
            let env_pdf = cosine_pdf(normal, wi).max(1.0e-6);
            DirectLightSample {
                wi,
                radiance,
                pdf: select_pdf * env_pdf.max(1.0e-6),
                max_t: f32::INFINITY,
                delta: false,
            }
        }
    }
}

fn cosine_sample_hemisphere(normal: Vec3, rng: &mut XorShift64) -> Vec3 {
    let u1 = rng.next_f32().clamp(1.0e-6, 1.0 - 1.0e-6);
    let u2 = rng.next_f32().clamp(1.0e-6, 1.0 - 1.0e-6);
    let r = u1.sqrt();
    let phi = 2.0 * std::f32::consts::PI * u2;
    let x = r * phi.cos();
    let y = r * phi.sin();
    let z = (1.0 - u1).sqrt();

    let n = normal.normalize();
    let tangent = if n.y.abs() < 0.99 {
        Vec3::new(0.0, 1.0, 0.0)
    } else {
        Vec3::new(1.0, 0.0, 0.0)
    };
    let t = tangent.cross(n).normalize();
    let b = n.cross(t).normalize();
    t.mul(x).add(b.mul(y)).add(n.mul(z)).normalize()
}

fn cosine_pdf(normal: Vec3, wi: Vec3) -> f32 {
    normal.dot(wi).max(0.0) / std::f32::consts::PI
}

fn shadow_visibility(
    accel: &(impl Accelerator + Sync),
    origin: Vec3,
    dir: Vec3,
    max_t: f32,
    epsilon: f32,
) -> f32 {
    let mut t = (epsilon * 4.0).max(1.0e-4);
    let mut visibility = 1.0_f32;
    for _ in 0..80 {
        if t >= max_t {
            return visibility.clamp(0.0, 1.0);
        }
        let p = origin.add(dir.mul(t));
        let h = accel.distance(p).abs();
        if h < epsilon * 4.0 {
            return 0.0;
        }
        visibility = visibility.min((10.0 * h / t).clamp(0.0, 1.0));
        t += h.max(epsilon * 2.0);
    }
    visibility.clamp(0.0, 1.0)
}

struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    fn new(seed: u64) -> Self {
        let state = if seed == 0 { 0x9E3779B97F4A7C15 } else { seed };
        Self { state }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    fn next_f32(&mut self) -> f32 {
        let v = (self.next_u64() >> 40) as u32;
        (v as f32) / ((1_u32 << 24) as f32)
    }
}

fn seed_pixel(x: u32, y: u32, width: u32) -> u64 {
    let idx = u64::from(y) * u64::from(width) + u64::from(x);
    idx.wrapping_mul(0x9E3779B97F4A7C15)
        .wrapping_add(0xBF58476D1CE4E5B9)
}

fn seed_pixel_sample(x: u32, y: u32, width: u32, sample: u32) -> u64 {
    let base = seed_pixel(x, y, width);
    let s = u64::from(sample).wrapping_mul(0x94D0_49BB_1331_11EB);
    base ^ s ^ 0xD6E8_FEB8_6659_FD93
}

#[derive(Clone, Copy)]
struct PixelAccumulator {
    sum: Spectrum,
    count: u32,
    mean_luma: f32,
    m2_luma: f32,
    active: bool,
}

impl PixelAccumulator {
    fn new() -> Self {
        Self {
            sum: Spectrum::black(),
            count: 0,
            mean_luma: 0.0,
            m2_luma: 0.0,
            active: true,
        }
    }

    fn add_sample(&mut self, sample: Spectrum) {
        self.sum = self.sum + sample;
        self.count += 1;
        let x = spectrum_luminance(sample);
        let delta = x - self.mean_luma;
        self.mean_luma += delta / self.count as f32;
        let delta2 = x - self.mean_luma;
        self.m2_luma += delta * delta2;
    }

    fn relative_error(&self) -> f32 {
        if self.count < 2 {
            return f32::INFINITY;
        }
        let n = self.count as f32;
        let variance = (self.m2_luma / (n - 1.0)).max(0.0);
        let stderr = (variance / n).sqrt();
        stderr / self.mean_luma.abs().max(1.0e-4)
    }
}

fn image_from_pixels(pixels: &[PixelAccumulator], width: u32, height: u32) -> RgbImage {
    let mut buffer = vec![0_u8; width as usize * height as usize * 3];
    for (idx, pixel) in pixels.iter().enumerate() {
        let avg = if pixel.count == 0 {
            Spectrum::black()
        } else {
            pixel.sum.scale(1.0 / pixel.count as f32)
        };
        let rgb = spectrum_to_rgb8(avg);
        let i = idx * 3;
        buffer[i] = rgb[0];
        buffer[i + 1] = rgb[1];
        buffer[i + 2] = rgb[2];
    }
    RgbImage::from_vec(width, height, buffer)
        .expect("pixel buffer length must match image dimensions")
}

#[derive(Clone, Copy)]
struct BsdfSample {
    wi: Vec3,
    f: Spectrum,
    pdf: f32,
    delta: bool,
    apply_cos: bool,
    transmission: bool,
    thin_walled: bool,
    next_ior: f32,
}

fn sample_bsdf_lobe(
    setup: &RenderSetup,
    mat: MaterialKindRt,
    bsdf_ctx: BsdfContextBase,
    rng: &mut XorShift64,
) -> BsdfSample {
    let u1 = rng.next_f32();
    let u2 = rng.next_f32();
    let u3 = rng.next_f32();
    if let Some(sampled) = sample_bsdf_ft(setup, mat, bsdf_ctx, u1, u2, u3) {
        return sampled;
    }
    let sampled = mat.sample(
        to_api_vec3(bsdf_ctx.normal),
        to_api_vec3(bsdf_ctx.wo),
        MaterialSampleInput {
            front_face: bsdf_ctx.hit.front_face,
            current_ior: bsdf_ctx.current_ior,
            u1,
            u2,
            u3,
        },
    );
    BsdfSample {
        wi: from_api_vec3(sampled.wi),
        f: sampled.f,
        pdf: sampled.pdf,
        delta: sampled.delta,
        apply_cos: sampled.apply_cos,
        transmission: sampled.transmission,
        thin_walled: sampled.thin_walled,
        next_ior: sampled.next_ior,
    }
}

fn eval_bsdf(
    setup: &RenderSetup,
    mat: MaterialKindRt,
    bsdf_ctx: BsdfContextBase,
    wi: Vec3,
) -> Spectrum {
    resolve_bsdf_spectrum_hook(setup, mat, "eval", bsdf_context_value(bsdf_ctx, wi, None))
        .unwrap_or_else(|| {
            mat.eval(
                to_api_vec3(bsdf_ctx.normal),
                to_api_vec3(wi),
                to_api_vec3(bsdf_ctx.wo),
            )
        })
}

fn pdf_bsdf(setup: &RenderSetup, mat: MaterialKindRt, bsdf_ctx: BsdfContextBase, wi: Vec3) -> f32 {
    resolve_bsdf_number_hook(setup, mat, "pdf", bsdf_context_value(bsdf_ctx, wi, None))
        .map(|v| v.max(0.0) as f32)
        .unwrap_or_else(|| {
            mat.pdf(
                to_api_vec3(bsdf_ctx.normal),
                to_api_vec3(wi),
                to_api_vec3(bsdf_ctx.wo),
            )
        })
}

fn sample_bsdf_ft(
    setup: &RenderSetup,
    mat: MaterialKindRt,
    bsdf_ctx: BsdfContextBase,
    u1: f32,
    u2: f32,
    u3: f32,
) -> Option<BsdfSample> {
    let value = resolve_bsdf_object_hook(
        setup,
        mat,
        "sample",
        bsdf_context_value(bsdf_ctx, bsdf_ctx.normal, Some((u1, u2, u3))),
    )?;
    bsdf_sample_from_value(&value)
}

fn resolve_bsdf_spectrum_hook(
    setup: &RenderSetup,
    material: MaterialKindRt,
    function_name: &str,
    ctx: Value,
) -> Option<Spectrum> {
    let name = dynamic_material_name(material, &setup.material_def_names)?;
    let value = eval_material_function(&setup.state, name, function_name, ctx).ok()?;
    spectrum_from_value(&value)
}

fn resolve_bsdf_number_hook(
    setup: &RenderSetup,
    material: MaterialKindRt,
    function_name: &str,
    ctx: Value,
) -> Option<f64> {
    let name = dynamic_material_name(material, &setup.material_def_names)?;
    let value = eval_material_function(&setup.state, name, function_name, ctx).ok()?;
    let Value::Number(v) = value else {
        return None;
    };
    Some(v)
}

fn resolve_bsdf_object_hook(
    setup: &RenderSetup,
    material: MaterialKindRt,
    function_name: &str,
    ctx: Value,
) -> Option<Value> {
    let name = dynamic_material_name(material, &setup.material_def_names)?;
    eval_material_function(&setup.state, name, function_name, ctx).ok()
}

fn dynamic_material_name(material: MaterialKindRt, material_def_names: &[String]) -> Option<&str> {
    let dynamic_material_id = match material {
        MaterialKindRt::Lambert(params)
        | MaterialKindRt::Metal(params)
        | MaterialKindRt::Dielectric(params) => params.dynamic_material_id,
    }?;
    material_def_names
        .get(dynamic_material_id as usize)
        .map(String::as_str)
}

fn bsdf_context_value(
    bsdf_ctx: BsdfContextBase,
    wi: Vec3,
    sample_randoms: Option<(f32, f32, f32)>,
) -> Value {
    let mut fields = match make_shading_context(bsdf_ctx.hit, bsdf_ctx.local_position, bsdf_ctx.wo)
    {
        Value::Object(obj) => obj.fields,
        Value::Number(_) => unreachable!(),
    };
    fields.insert("normal".to_string(), vec3_value_value(bsdf_ctx.normal));
    fields.insert("wo".to_string(), vec3_value_value(bsdf_ctx.wo));
    fields.insert("wi".to_string(), vec3_value_value(wi));
    fields.insert(
        "current_ior".to_string(),
        Value::Number(bsdf_ctx.current_ior as f64),
    );
    if let Some((u1, u2, u3)) = sample_randoms {
        fields.insert("u1".to_string(), Value::Number(u1 as f64));
        fields.insert("u2".to_string(), Value::Number(u2 as f64));
        fields.insert("u3".to_string(), Value::Number(u3 as f64));
    }
    Value::Object(ObjectValue {
        type_name: None,
        fields,
    })
}

fn bsdf_sample_from_value(value: &Value) -> Option<BsdfSample> {
    let Value::Object(obj) = value else {
        return None;
    };
    let wi = obj.fields.get("wi").and_then(vec3_from_value)?;
    let f = obj
        .fields
        .get("f")
        .or_else(|| obj.fields.get("color"))
        .and_then(spectrum_from_value)?;
    let pdf = read_number_field(obj, &["pdf"]).unwrap_or(0.0).max(0.0);
    let delta = read_number_field(obj, &["delta"]).unwrap_or(0.0) >= 0.5;
    let apply_cos = read_number_field(obj, &["apply_cos"]).unwrap_or(1.0) >= 0.5;
    let transmission = read_number_field(obj, &["transmission"]).unwrap_or(0.0) >= 0.5;
    let thin_walled = read_number_field(obj, &["thin_walled"]).unwrap_or(0.0) >= 0.5;
    let next_ior = read_number_field(obj, &["next_ior"])
        .unwrap_or(1.0)
        .clamp(1.0, 3.0);
    Some(BsdfSample {
        wi,
        f,
        pdf,
        delta,
        apply_cos,
        transmission,
        thin_walled,
        next_ior,
    })
}

fn reflect(v: Vec3, n: Vec3) -> Vec3 {
    v.sub(n.mul(2.0 * v.dot(n)))
}

fn refract(incident: Vec3, normal: Vec3, eta: f32) -> Option<Vec3> {
    let i = incident.normalize();
    let mut n = normal.normalize();
    let mut cosi = i.dot(n).clamp(-1.0, 1.0);
    if cosi > 0.0 {
        n = n.mul(-1.0);
    } else {
        cosi = -cosi;
    }
    let sin2_t = eta * eta * (1.0 - cosi * cosi);
    if sin2_t > 1.0 {
        return None;
    }
    let cost = (1.0 - sin2_t).sqrt();
    Some(i.mul(eta).add(n.mul(eta * cosi - cost)).normalize())
}

fn power_heuristic(pa: f32, pb: f32) -> f32 {
    let a2 = pa * pa;
    let b2 = pb * pb;
    a2 / (a2 + b2).max(1.0e-6)
}

fn spectrum_luminance(s: Spectrum) -> f32 {
    0.2126 * s.r + 0.7152 * s.g + 0.0722 * s.b
}

fn medium_transmittance(medium: MediumState, distance: f32) -> Spectrum {
    if medium.density <= 0.0 || distance <= 0.0 {
        return Spectrum::rgb(1.0, 1.0, 1.0);
    }
    let exponent = medium.density * distance.max(0.0);
    Spectrum::rgb(
        medium.absorption_color.r.clamp(0.0, 1.0).powf(exponent),
        medium.absorption_color.g.clamp(0.0, 1.0).powf(exponent),
        medium.absorption_color.b.clamp(0.0, 1.0).powf(exponent),
    )
}

fn apply_medium_attenuation(color: Spectrum, medium: MediumState, distance: f32) -> Spectrum {
    color * medium_transmittance(medium, distance)
}

fn medium_state_from_material(material: MaterialKindRt) -> Option<MediumState> {
    let params = match material {
        MaterialKindRt::Lambert(params)
        | MaterialKindRt::Metal(params)
        | MaterialKindRt::Dielectric(params) => params,
    };
    let explicit = params.medium.map(|medium| MediumState {
        ior: medium.ior.clamp(1.0, 3.0),
        absorption_color: medium.absorption_color,
        density: medium.density.max(0.0),
    });
    explicit.or_else(|| {
        matches!(material, MaterialKindRt::Dielectric(_)).then_some(MediumState {
            ior: params.ior.clamp(1.0, 3.0),
            absorption_color: Spectrum::rgb(1.0, 1.0, 1.0),
            density: 0.0,
        })
    })
}

fn transition_medium(
    material: MaterialKindRt,
    front_face: bool,
    current: MediumState,
) -> MediumState {
    if !matches!(material, MaterialKindRt::Dielectric(_)) {
        return current;
    }
    if front_face {
        medium_state_from_material(material).unwrap_or(current)
    } else {
        MediumState::air()
    }
}

fn clamp_spectrum(s: Spectrum, max_luma: f32) -> Spectrum {
    let l = spectrum_luminance(s);
    if l <= max_luma || l <= 1.0e-6 {
        s
    } else {
        s.scale(max_luma / l)
    }
}

fn fresnel_f0_from_ior(ior: f32) -> f32 {
    let i = ior.max(1.0e-3);
    let r = (i - 1.0) / (i + 1.0);
    r * r
}

fn fresnel_schlick_scalar(f0: f32, cos_theta: f32) -> f32 {
    let m = (1.0 - cos_theta.clamp(0.0, 1.0)).powi(5);
    f0 + (1.0 - f0) * m
}

fn fresnel_dielectric_scalar(cos_theta_i: f32, eta_i: f32, eta_t: f32) -> f32 {
    let ei = eta_i.max(1.0e-4);
    let et = eta_t.max(1.0e-4);
    if (ei - et).abs() < 1.0e-6 {
        return 0.0;
    }
    let ci = cos_theta_i.clamp(0.0, 1.0);
    let eta = ei / et;
    let sin2_t = eta * eta * (1.0 - ci * ci);
    if sin2_t >= 1.0 {
        return 1.0;
    }
    let ct = (1.0 - sin2_t).sqrt();
    let rs = ((ei * ci) - (et * ct)) / ((ei * ci) + (et * ct)).max(1.0e-6);
    let rp = ((et * ci) - (ei * ct)) / ((et * ci) + (ei * ct)).max(1.0e-6);
    0.5 * (rs * rs + rp * rp)
}

fn lerp_spectrum(a: Spectrum, b: Spectrum, t: f32) -> Spectrum {
    let tt = t.clamp(0.0, 1.0);
    Spectrum::rgb(
        a.r * (1.0 - tt) + b.r * tt,
        a.g * (1.0 - tt) + b.g * tt,
        a.b * (1.0 - tt) + b.b * tt,
    )
}

fn spectrum_to_rgb8(s: Spectrum) -> [u8; 3] {
    fn to_u8(v: f32) -> u8 {
        let mapped = v.max(0.0).powf(1.0 / 2.2).min(1.0);
        (mapped * 255.0) as u8
    }
    [to_u8(s.r), to_u8(s.g), to_u8(s.b)]
}

fn spectrum_to_rgb8_reinhard(s: Spectrum) -> [u8; 3] {
    fn tone(v: f32) -> f32 {
        let x = v.max(0.0);
        x / (1.0 + x)
    }
    fn to_u8(v: f32) -> u8 {
        let mapped = tone(v).powf(1.0 / 2.2).min(1.0);
        (mapped * 255.0) as u8
    }
    [to_u8(s.r), to_u8(s.g), to_u8(s.b)]
}

fn distance_to_aabb(p: Vec3, bounds: Aabb) -> f32 {
    let dx = (bounds.min.x - p.x).max(0.0).max(p.x - bounds.max.x);
    let dy = (bounds.min.y - p.y).max(0.0).max(p.y - bounds.max.y);
    let dz = (bounds.min.z - p.z).max(0.0).max(p.z - bounds.max.z);
    Vec3::new(dx, dy, dz).length()
}

fn sdf_bounds(node: &SdfNode) -> Aabb {
    match node {
        SdfNode::Sphere {
            transform, radius, ..
        } => {
            let r = Vec3::new(*radius, *radius, *radius);
            Aabb {
                min: transform.center.sub(r),
                max: transform.center.add(r),
            }
        }
        SdfNode::Box {
            transform,
            half_size,
            ..
        } => {
            let r = half_size.length();
            let rv = Vec3::new(r, r, r);
            Aabb {
                min: transform.center.sub(rv),
                max: transform.center.add(rv),
            }
        }
        SdfNode::Cylinder {
            transform,
            radius,
            half_height,
            ..
        } => {
            let r = (radius * radius + half_height * half_height).sqrt();
            let rv = Vec3::new(r, r, r);
            Aabb {
                min: transform.center.sub(rv),
                max: transform.center.add(rv),
            }
        }
        SdfNode::Torus {
            transform,
            major_radius,
            minor_radius,
            ..
        } => {
            let r = major_radius + minor_radius;
            let rv = Vec3::new(r, r, r);
            Aabb {
                min: transform.center.sub(rv),
                max: transform.center.add(rv),
            }
        }
        SdfNode::Custom {
            transform,
            bounds_radius,
            ..
        } => {
            let r = (*bounds_radius).max(1.0e-3);
            let rv = Vec3::new(r, r, r);
            Aabb {
                min: transform.center.sub(rv),
                max: transform.center.add(rv),
            }
        }
        SdfNode::Union { lhs, rhs } => sdf_bounds(lhs).union(sdf_bounds(rhs)),
        SdfNode::Subtract { lhs, .. } => sdf_bounds(lhs),
        SdfNode::Smooth { base, k } => sdf_bounds(base).expand(*k * 0.1),
        SdfNode::Round { base, r } => sdf_bounds(base).expand(r.abs()),
    }
}

fn sdf_distance_info(node: &SdfNode, p: Vec3) -> DistanceInfo {
    match node {
        SdfNode::Sphere {
            transform,
            radius,
            object_id,
            material_id,
        } => {
            let q = to_local(p, *transform);
            DistanceInfo {
                distance: q.length() - *radius,
                object_id: *object_id,
                material_id: *material_id,
            }
        }
        SdfNode::Box {
            transform,
            half_size,
            object_id,
            material_id,
        } => {
            let q = to_local(p, *transform).abs().sub(*half_size);
            let outside = Vec3::new(q.x.max(0.0), q.y.max(0.0), q.z.max(0.0)).length();
            let inside = q.x.max(q.y).max(q.z).min(0.0);
            DistanceInfo {
                distance: outside + inside,
                object_id: *object_id,
                material_id: *material_id,
            }
        }
        SdfNode::Cylinder {
            transform,
            radius,
            half_height,
            object_id,
            material_id,
        } => {
            let q = to_local(p, *transform);
            let radial = (q.x * q.x + q.z * q.z).sqrt();
            let dx = radial - *radius;
            let dy = q.y.abs() - *half_height;
            let outside = (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt();
            let inside = dx.max(dy).min(0.0);
            DistanceInfo {
                distance: outside + inside,
                object_id: *object_id,
                material_id: *material_id,
            }
        }
        SdfNode::Torus {
            transform,
            major_radius,
            minor_radius,
            object_id,
            material_id,
        } => {
            let q = to_local(p, *transform);
            let qx = (q.x * q.x + q.z * q.z).sqrt() - *major_radius;
            DistanceInfo {
                distance: (qx * qx + q.y * q.y).sqrt() - *minor_radius,
                object_id: *object_id,
                material_id: *material_id,
            }
        }
        SdfNode::Custom {
            transform,
            runtime,
            object_id,
            material_id,
            ..
        } => {
            let q = to_local(p, *transform);
            DistanceInfo {
                distance: eval_custom_sdf_distance(runtime, q),
                object_id: *object_id,
                material_id: *material_id,
            }
        }
        SdfNode::Union { lhs, rhs } => {
            let l = sdf_distance_info(lhs, p);
            let r = sdf_distance_info(rhs, p);
            if l.distance <= r.distance { l } else { r }
        }
        SdfNode::Subtract { lhs, rhs } => {
            let l = sdf_distance_info(lhs, p);
            let r = sdf_distance_info(rhs, p);
            let rd = -r.distance;
            if l.distance >= rd {
                l
            } else {
                DistanceInfo {
                    distance: rd,
                    object_id: r.object_id,
                    material_id: r.material_id,
                }
            }
        }
        SdfNode::Smooth { base, k } => {
            let mut info = sdf_distance_info(base, p);
            info.distance -= *k * 0.1;
            info
        }
        SdfNode::Round { base, r } => {
            let mut info = sdf_distance_info(base, p);
            info.distance -= *r;
            info
        }
    }
}

fn eval_custom_sdf_distance(runtime: &CustomSdfRuntime, p: Vec3) -> f32 {
    let value = eval_sdf_function(
        &runtime.state,
        &runtime.name,
        "distance",
        vec3_value_value(p),
    );
    match value {
        Ok(Value::Number(v)) => v as f32,
        _ => 1.0e6,
    }
}

fn eval_custom_sdf_bounds_radius(state: &EvalState, name: &str) -> f32 {
    let value = eval_sdf_zero_arg_function(state, name, "bounds");
    match value {
        Ok(Value::Object(obj)) => {
            let x = read_number_field(&obj, &["x"]).unwrap_or(0.0);
            let y = read_number_field(&obj, &["y"]).unwrap_or(0.0);
            let z = read_number_field(&obj, &["z"]).unwrap_or(0.0);
            Vec3::new(x, y, z).length().max(1.0e-3)
        }
        Ok(Value::Number(radius)) => (radius as f32).abs().max(1.0e-3),
        _ => 10_000.0,
    }
}

fn to_local(p: Vec3, transform: PrimitiveTransform) -> Vec3 {
    let mut q = p.sub(transform.center);
    q = rotate_x(q, -transform.rot_deg.x);
    q = rotate_y(q, -transform.rot_deg.y);
    q = rotate_z(q, -transform.rot_deg.z);
    q
}

fn rotate_x(v: Vec3, deg: f32) -> Vec3 {
    let r = deg.to_radians();
    let (s, c) = r.sin_cos();
    Vec3::new(v.x, c * v.y - s * v.z, s * v.y + c * v.z)
}

fn rotate_y(v: Vec3, deg: f32) -> Vec3 {
    let r = deg.to_radians();
    let (s, c) = r.sin_cos();
    Vec3::new(c * v.x + s * v.z, v.y, -s * v.x + c * v.z)
}

fn rotate_z(v: Vec3, deg: f32) -> Vec3 {
    let r = deg.to_radians();
    let (s, c) = r.sin_cos();
    Vec3::new(c * v.x - s * v.y, s * v.x + c * v.y, v.z)
}

fn build_render_setup(
    state: &EvalState,
    scene: &CompiledScene,
    options: RenderOptions,
) -> RenderSetup {
    let camera = parse_camera(state, scene.center, options);
    let (lights, path_lights) = parse_lights(state);
    RenderSetup {
        state: state.clone(),
        camera,
        lights,
        path_lights,
        materials: scene.materials.clone(),
        object_transforms: scene.object_transforms.clone(),
        material_def_names: sorted_material_def_names(state),
    }
}

fn parse_camera(state: &EvalState, scene_center: Vec3, options: RenderOptions) -> CameraKind {
    if let Some(binding) = state.bindings.get("camera")
        && let Value::Object(camera_obj) = &binding.value
    {
        let origin = read_vec3_field(camera_obj, "origin")
            .or_else(|| read_vec3_field(camera_obj, "position"))
            .unwrap_or_else(|| scene_center.add(Vec3::new(0.0, 0.0, options.camera_z)));
        let target = read_vec3_field(camera_obj, "target").unwrap_or(scene_center);
        let fov = read_number_field(camera_obj, &["fov_y", "fov"]).unwrap_or(options.fov_y_degrees);
        return CameraKind::Pinhole(PinholeCamera {
            origin: to_api_vec3(origin),
            target: to_api_vec3(target),
            up: ApiVec3::new(0.0, 1.0, 0.0),
            fov_y_degrees: fov,
        });
    }

    CameraKind::Pinhole(PinholeCamera {
        origin: to_api_vec3(scene_center.add(Vec3::new(0.0, 0.0, options.camera_z))),
        target: to_api_vec3(scene_center),
        up: ApiVec3::new(0.0, 1.0, 0.0),
        fov_y_degrees: options.fov_y_degrees,
    })
}

fn parse_lights(state: &EvalState) -> (Vec<Box<dyn Light>>, Vec<PathLight>) {
    let mut lights: Vec<Box<dyn Light>> = Vec::new();
    let mut path_lights: Vec<PathLight> = Vec::new();
    for binding in state.bindings.values() {
        let Value::Object(obj) = &binding.value else {
            continue;
        };
        let Some(type_name) = obj.type_name.as_deref() else {
            continue;
        };
        match type_name {
            "PointLight" => {
                let position = read_vec3_field(obj, "position").unwrap_or_else(|| read_center(obj));
                let intensity = read_spectrum_field(obj, "intensity")
                    .or_else(|| read_spectrum_field(obj, "color"))
                    .unwrap_or(Spectrum::rgb(8.0, 8.0, 8.0));
                lights.push(Box::new(PointLight {
                    position: to_api_vec3(position),
                    intensity,
                }));
                path_lights.push(PathLight::Point {
                    position: to_api_vec3(position),
                    intensity,
                });
            }
            "EnvLight" => {
                let radiance = read_spectrum_field(obj, "radiance")
                    .or_else(|| read_spectrum_field(obj, "color"))
                    .unwrap_or(Spectrum::rgb(0.15, 0.15, 0.15));
                lights.push(Box::new(EnvLight { radiance }));
                path_lights.push(PathLight::Env { radiance });
            }
            _ => {}
        }
    }

    if lights.is_empty() {
        let p = PathLight::Point {
            position: ApiVec3::new(3.0, 3.0, 6.0),
            intensity: Spectrum::rgb(6.0, 6.0, 6.0),
        };
        let e = PathLight::Env {
            radiance: Spectrum::rgb(0.1, 0.1, 0.1),
        };
        path_lights.push(p);
        path_lights.push(e);
        let PathLight::Point {
            position,
            intensity,
        } = p
        else {
            unreachable!();
        };
        let PathLight::Env { radiance } = e else {
            unreachable!();
        };
        lights.push(Box::new(PointLight {
            position,
            intensity,
        }));
        lights.push(Box::new(EnvLight { radiance }));
    }

    (lights, path_lights)
}

fn parse_material(state: &EvalState, scene_root: &Value) -> MaterialKindRt {
    if let Some(binding) = state.bindings.get("material")
        && let Some(material) = extract_material_kind(state, &binding.value)
    {
        return material;
    }

    if let Some(material) = extract_material_kind(state, scene_root) {
        return material;
    }

    default_material()
}

fn extract_material_kind(state: &EvalState, value: &Value) -> Option<MaterialKindRt> {
    let Value::Object(obj) = value else {
        return None;
    };

    if let Some(material_value) = obj.fields.get("material")
        && let Value::Object(material_obj) = material_value
    {
        return Some(material_from_object(state, material_obj));
    }

    for field_value in obj.fields.values() {
        if let Some(found) = extract_material_kind(state, field_value) {
            return Some(found);
        }
    }

    let type_name = obj.type_name.as_deref().unwrap_or_default();
    if type_name.eq_ignore_ascii_case("material")
        || type_name.eq_ignore_ascii_case("openpbr")
        || type_name.eq_ignore_ascii_case("lambert")
        || type_name.eq_ignore_ascii_case("metal")
        || type_name.eq_ignore_ascii_case("dielectric")
    {
        return Some(material_from_object(state, obj));
    }

    if state.material_defs.contains_key(type_name) {
        return Some(material_from_dynamic_def(state, type_name));
    }

    None
}

fn material_from_object(state: &EvalState, obj: &ObjectValue) -> MaterialKindRt {
    let type_name = obj.type_name.as_deref().unwrap_or_default();
    if state.material_defs.contains_key(type_name) {
        return material_from_dynamic_def(state, type_name);
    }
    if type_name.eq_ignore_ascii_case("lambert") {
        let color = read_spectrum_field(obj, "color")
            .or_else(|| read_spectrum_field(obj, "base_color"))
            .unwrap_or(Spectrum::rgb(0.8, 0.8, 0.8));
        let emission_color =
            read_spectrum_field(obj, "emission_color").unwrap_or(Spectrum::black());
        let emission_strength = read_number_field(obj, &["emission_strength"])
            .unwrap_or(0.0)
            .max(0.0);
        let mut params = MaterialParams::lambert(color, emission_color, emission_strength);
        params.medium = read_medium_field(obj, "medium");
        params.subsurface = read_subsurface_field(obj, "subsurface");
        params.pattern = read_pattern_field(obj, "pattern");
        return MaterialKindRt::Lambert(params);
    }
    if type_name.eq_ignore_ascii_case("metal") {
        let color = read_spectrum_field(obj, "color")
            .or_else(|| read_spectrum_field(obj, "base_color"))
            .unwrap_or(Spectrum::rgb(0.9, 0.9, 0.9));
        let roughness = read_number_field(obj, &["roughness"])
            .unwrap_or(0.1)
            .clamp(0.0, 1.0);
        let emission_color =
            read_spectrum_field(obj, "emission_color").unwrap_or(Spectrum::black());
        let emission_strength = read_number_field(obj, &["emission_strength"])
            .unwrap_or(0.0)
            .max(0.0);
        let mut params = MaterialParams::metal(color, roughness, emission_color, emission_strength);
        params.medium = read_medium_field(obj, "medium");
        params.subsurface = read_subsurface_field(obj, "subsurface");
        params.pattern = read_pattern_field(obj, "pattern");
        return MaterialKindRt::Metal(params);
    }
    if type_name.eq_ignore_ascii_case("dielectric") {
        let color = read_spectrum_field(obj, "color")
            .or_else(|| read_spectrum_field(obj, "base_color"))
            .unwrap_or(Spectrum::rgb(1.0, 1.0, 1.0));
        let ior = read_number_field(obj, &["ior"])
            .unwrap_or(1.5)
            .clamp(1.0, 3.0);
        let roughness = read_number_field(obj, &["roughness"])
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);
        let thin_walled = read_number_field(obj, &["thin_walled"]).unwrap_or(0.0) >= 0.5;
        let emission_color =
            read_spectrum_field(obj, "emission_color").unwrap_or(Spectrum::black());
        let emission_strength = read_number_field(obj, &["emission_strength"])
            .unwrap_or(0.0)
            .max(0.0);
        let mut params = MaterialParams::dielectric(
            color,
            ior,
            roughness,
            thin_walled,
            emission_color,
            emission_strength,
        );
        params.medium = read_medium_field(obj, "medium");
        params.subsurface = read_subsurface_field(obj, "subsurface");
        params.pattern = read_pattern_field(obj, "pattern");
        return MaterialKindRt::Dielectric(params);
    }
    material_from_legacy_object(state, obj)
}

fn material_from_legacy_object(_state: &EvalState, obj: &ObjectValue) -> MaterialKindRt {
    let color = read_spectrum_field(obj, "base_color")
        .or_else(|| read_spectrum_field(obj, "color"))
        .or_else(|| read_spectrum_field(obj, "albedo"))
        .unwrap_or(Spectrum::rgb(0.8, 0.8, 0.8));
    let roughness = read_number_field(obj, &["roughness"])
        .unwrap_or(0.5)
        .clamp(0.0, 1.0);
    let metallic = read_number_field(obj, &["metallic", "metalness"])
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    let transmission = read_number_field(obj, &["transmission", "transmission_weight"])
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    let ior = read_number_field(obj, &["ior", "specular_ior"])
        .unwrap_or(1.5)
        .clamp(1.0, 3.0);
    let thin_walled = read_number_field(obj, &["thin_walled", "thinwall"]).unwrap_or(0.0) >= 0.5;
    let emission_color = read_spectrum_field(obj, "emission_color")
        .or_else(|| read_spectrum_field(obj, "emission"))
        .unwrap_or(Spectrum::black());
    let emission_strength = read_number_field(obj, &["emission_strength", "emission_weight"])
        .unwrap_or(0.0)
        .max(0.0);

    if transmission > 0.01 {
        let mut params = MaterialParams::dielectric(
            color,
            ior,
            roughness,
            thin_walled,
            emission_color,
            emission_strength,
        );
        params.medium = read_medium_field(obj, "medium");
        params.subsurface = read_subsurface_field(obj, "subsurface");
        params.pattern = read_pattern_field(obj, "pattern");
        return MaterialKindRt::Dielectric(params);
    }
    if metallic > 0.01 {
        let mut params = MaterialParams::metal(color, roughness, emission_color, emission_strength);
        params.medium = read_medium_field(obj, "medium");
        params.subsurface = read_subsurface_field(obj, "subsurface");
        params.pattern = read_pattern_field(obj, "pattern");
        return MaterialKindRt::Metal(params);
    }
    let mut params = MaterialParams::lambert(color, emission_color, emission_strength);
    params.medium = read_medium_field(obj, "medium");
    params.subsurface = read_subsurface_field(obj, "subsurface");
    params.pattern = read_pattern_field(obj, "pattern");
    MaterialKindRt::Lambert(params)
}

fn material_from_dynamic_def(state: &EvalState, name: &str) -> MaterialKindRt {
    let Some(def) = state.material_defs.get(name) else {
        return default_material();
    };
    let mut params = match def.model.as_str() {
        "Metal" => MaterialParams::metal(Spectrum::rgb(0.9, 0.9, 0.9), 0.1, Spectrum::black(), 0.0),
        "Dielectric" => MaterialParams::dielectric(
            Spectrum::rgb(1.0, 1.0, 1.0),
            1.5,
            0.0,
            false,
            Spectrum::black(),
            0.0,
        ),
        _ => MaterialParams::lambert(Spectrum::rgb(0.8, 0.8, 0.8), Spectrum::black(), 0.0),
    };
    apply_dynamic_material_properties(state, name, &mut params);
    params.dynamic_material_id = sorted_material_def_names(state)
        .iter()
        .position(|candidate| candidate == name)
        .map(|idx| idx as u32);
    match def.model.as_str() {
        "Metal" => MaterialKindRt::Metal(params),
        "Dielectric" => MaterialKindRt::Dielectric(params),
        _ => MaterialKindRt::Lambert(params),
    }
}

fn apply_dynamic_material_properties(
    state: &EvalState,
    material_name: &str,
    params: &mut MaterialParams,
) {
    let Ok(properties) = eval_material_properties(state, material_name) else {
        return;
    };
    for (name, value) in properties {
        match name.as_str() {
            "color" => {
                if let Some(color) = spectrum_from_value(&value) {
                    params.color = color;
                }
            }
            "roughness" => {
                if let Value::Number(v) = value {
                    params.roughness = (v as f32).clamp(0.0, 1.0);
                }
            }
            "ior" => {
                if let Value::Number(v) = value {
                    params.ior = (v as f32).clamp(1.0, 3.0);
                }
            }
            "thin_walled" => {
                if let Value::Number(v) = value {
                    params.thin_walled = v >= 0.5;
                }
            }
            "emission_color" => {
                if let Some(color) = spectrum_from_value(&value) {
                    params.emission_color = color;
                }
            }
            "emission_strength" => {
                if let Value::Number(v) = value {
                    params.emission_strength = (v as f32).max(0.0);
                }
            }
            "medium" => {
                params.medium = medium_from_value(&value);
            }
            "subsurface" => {
                params.subsurface = subsurface_from_value(&value);
            }
            _ => {}
        }
    }
}

fn read_spectrum_field(obj: &ObjectValue, name: &str) -> Option<Spectrum> {
    let value = obj.fields.get(name)?;
    match value {
        Value::Number(v) => Some(Spectrum::rgb(*v as f32, *v as f32, *v as f32)),
        Value::Object(v) => {
            let r = read_number_field(v, &["r", "x"])?;
            let g = read_number_field(v, &["g", "y"])?;
            let b = read_number_field(v, &["b", "z"])?;
            Some(Spectrum::rgb(r, g, b))
        }
    }
}

fn read_pattern_field(obj: &ObjectValue, name: &str) -> Option<ColorPattern> {
    let Value::Object(pattern) = obj.fields.get(name)? else {
        return None;
    };
    let kind = pattern.type_name.as_deref().unwrap_or_default();
    if kind.eq_ignore_ascii_case("checker3d") || kind.eq_ignore_ascii_case("checker") {
        let color_a = read_spectrum_field(pattern, "color_a")
            .or_else(|| read_spectrum_field(pattern, "color1"))
            .or_else(|| read_spectrum_field(pattern, "a"))
            .unwrap_or(Spectrum::rgb(0.15, 0.15, 0.15));
        let color_b = read_spectrum_field(pattern, "color_b")
            .or_else(|| read_spectrum_field(pattern, "color2"))
            .or_else(|| read_spectrum_field(pattern, "b"))
            .unwrap_or(Spectrum::rgb(0.85, 0.85, 0.85));
        let scale = read_number_field(pattern, &["scale", "frequency", "freq"]).unwrap_or(4.0);
        return Some(ColorPattern::Checker3d {
            color_a,
            color_b,
            scale: scale.max(1.0e-4),
        });
    }
    None
}

fn read_medium_field(obj: &ObjectValue, name: &str) -> Option<MediumParams> {
    medium_from_value(obj.fields.get(name)?)
}

fn read_subsurface_field(obj: &ObjectValue, name: &str) -> Option<SubsurfaceParams> {
    subsurface_from_value(obj.fields.get(name)?)
}

fn medium_from_value(value: &Value) -> Option<MediumParams> {
    let Value::Object(obj) = value else {
        return None;
    };
    let ior = read_number_field(obj, &["ior"])
        .unwrap_or(1.0)
        .clamp(1.0, 3.0);
    let absorption_color =
        read_spectrum_field(obj, "absorption_color").unwrap_or(Spectrum::rgb(1.0, 1.0, 1.0));
    let density = read_number_field(obj, &["density"]).unwrap_or(0.0).max(0.0);
    Some(MediumParams::new(ior, absorption_color, density))
}

fn subsurface_from_value(value: &Value) -> Option<SubsurfaceParams> {
    let Value::Object(obj) = value else {
        return None;
    };
    let color = read_spectrum_field(obj, "color")
        .or_else(|| read_spectrum_field(obj, "scatter_color"))
        .unwrap_or(Spectrum::rgb(1.0, 1.0, 1.0));
    let radius = obj
        .fields
        .get("radius")
        .and_then(vec3_from_value)
        .unwrap_or_else(|| Vec3::new(1.0, 1.0, 1.0));
    let anisotropy = read_number_field(obj, &["anisotropy", "g"])
        .unwrap_or(0.0)
        .clamp(-0.99, 0.99);
    let scale = read_number_field(obj, &["scale"]).unwrap_or(1.0).max(0.0);
    Some(SubsurfaceParams::new(
        color,
        to_api_vec3(radius),
        anisotropy,
        scale,
    ))
}

fn vec3_from_value(value: &Value) -> Option<Vec3> {
    match value {
        Value::Number(v) => {
            let f = *v as f32;
            Some(Vec3::new(f, f, f))
        }
        Value::Object(obj) => {
            let x = read_number_field(obj, &["x", "r"])?;
            let y = read_number_field(obj, &["y", "g"])?;
            let z = read_number_field(obj, &["z", "b"])?;
            Some(Vec3::new(x, y, z))
        }
    }
}

fn to_api_vec3(v: Vec3) -> ApiVec3 {
    ApiVec3::new(v.x, v.y, v.z)
}

fn from_api_vec3(v: ApiVec3) -> Vec3 {
    Vec3::new(v.x, v.y, v.z)
}

fn material_for_id(materials: &[MaterialKindRt], material_id: u32) -> MaterialKindRt {
    materials
        .get(material_id as usize)
        .copied()
        .unwrap_or_else(|| materials.first().copied().unwrap_or(default_material()))
}

fn sorted_material_def_names(state: &EvalState) -> Vec<String> {
    let mut names: Vec<_> = state.material_defs.keys().cloned().collect();
    names.sort();
    names
}

fn resolve_material_at_hit(setup: &RenderSetup, hit: RayHit, view_dir: Vec3) -> MaterialKindRt {
    let material = material_for_id(&setup.materials, hit.material_id);
    let transform = setup
        .object_transforms
        .get(hit.object_id as usize)
        .copied()
        .unwrap_or_else(PrimitiveTransform::identity);
    let local_position = to_local(hit.position, transform);
    let runtime_color = resolve_dynamic_color(
        &setup.state,
        &setup.material_def_names,
        material,
        hit,
        local_position,
        view_dir,
    );
    let runtime_roughness = resolve_dynamic_number(
        &setup.state,
        &setup.material_def_names,
        material,
        hit,
        local_position,
        view_dir,
        "roughness",
    );
    let runtime_ior = resolve_dynamic_number(
        &setup.state,
        &setup.material_def_names,
        material,
        hit,
        local_position,
        view_dir,
        "ior",
    );
    let runtime_thin_walled = resolve_dynamic_number(
        &setup.state,
        &setup.material_def_names,
        material,
        hit,
        local_position,
        view_dir,
        "thin_walled",
    );
    let runtime_medium = resolve_dynamic_object(
        &setup.state,
        &setup.material_def_names,
        material,
        hit,
        local_position,
        view_dir,
        "medium",
    )
    .and_then(|value| medium_from_value(&value));
    let runtime_subsurface = resolve_dynamic_object(
        &setup.state,
        &setup.material_def_names,
        material,
        hit,
        local_position,
        view_dir,
        "subsurface",
    )
    .and_then(|value| subsurface_from_value(&value));
    let runtime_emission_color = resolve_dynamic_spectrum(
        &setup.state,
        &setup.material_def_names,
        material,
        hit,
        local_position,
        view_dir,
        "emission_color",
    );
    let runtime_emission_strength = resolve_dynamic_number(
        &setup.state,
        &setup.material_def_names,
        material,
        hit,
        local_position,
        view_dir,
        "emission_strength",
    );
    match material {
        MaterialKindRt::Lambert(mut params) => {
            params.color =
                runtime_color.unwrap_or_else(|| resolve_pattern_color(params, local_position));
            params.medium = runtime_medium.or(params.medium);
            params.subsurface = runtime_subsurface.or(params.subsurface);
            params.emission_color = runtime_emission_color.unwrap_or(params.emission_color);
            params.emission_strength = runtime_emission_strength
                .unwrap_or(params.emission_strength as f64)
                .max(0.0) as f32;
            MaterialKindRt::Lambert(params)
        }
        MaterialKindRt::Metal(mut params) => {
            params.color =
                runtime_color.unwrap_or_else(|| resolve_pattern_color(params, local_position));
            params.roughness = runtime_roughness
                .unwrap_or(params.roughness as f64)
                .clamp(0.0, 1.0) as f32;
            params.medium = runtime_medium.or(params.medium);
            params.subsurface = runtime_subsurface.or(params.subsurface);
            params.emission_color = runtime_emission_color.unwrap_or(params.emission_color);
            params.emission_strength = runtime_emission_strength
                .unwrap_or(params.emission_strength as f64)
                .max(0.0) as f32;
            MaterialKindRt::Metal(params)
        }
        MaterialKindRt::Dielectric(mut params) => {
            params.color =
                runtime_color.unwrap_or_else(|| resolve_pattern_color(params, local_position));
            params.roughness = runtime_roughness
                .unwrap_or(params.roughness as f64)
                .clamp(0.0, 1.0) as f32;
            params.ior = runtime_ior.unwrap_or(params.ior as f64).clamp(1.0, 3.0) as f32;
            if let Some(thin_walled) = runtime_thin_walled {
                params.thin_walled = thin_walled >= 0.5;
            }
            params.medium = runtime_medium.or(params.medium);
            params.subsurface = runtime_subsurface.or(params.subsurface);
            params.emission_color = runtime_emission_color.unwrap_or(params.emission_color);
            params.emission_strength = runtime_emission_strength
                .unwrap_or(params.emission_strength as f64)
                .max(0.0) as f32;
            MaterialKindRt::Dielectric(params)
        }
    }
}

fn resolve_dynamic_color(
    state: &EvalState,
    material_def_names: &[String],
    material: MaterialKindRt,
    hit: RayHit,
    local_position: Vec3,
    view_dir: Vec3,
) -> Option<Spectrum> {
    let dynamic_material_id = match material {
        MaterialKindRt::Lambert(params)
        | MaterialKindRt::Metal(params)
        | MaterialKindRt::Dielectric(params) => params.dynamic_material_id,
    }?;
    let name = material_def_names.get(dynamic_material_id as usize)?;
    let ctx = make_shading_context(hit, local_position, view_dir);
    let value = eval_material_function(state, name, "color", ctx).ok()?;
    spectrum_from_value(&value)
}

fn resolve_dynamic_spectrum(
    state: &EvalState,
    material_def_names: &[String],
    material: MaterialKindRt,
    hit: RayHit,
    local_position: Vec3,
    view_dir: Vec3,
    function_name: &str,
) -> Option<Spectrum> {
    let dynamic_material_id = match material {
        MaterialKindRt::Lambert(params)
        | MaterialKindRt::Metal(params)
        | MaterialKindRt::Dielectric(params) => params.dynamic_material_id,
    }?;
    let name = material_def_names.get(dynamic_material_id as usize)?;
    let ctx = make_shading_context(hit, local_position, view_dir);
    let value = eval_material_function(state, name, function_name, ctx).ok()?;
    spectrum_from_value(&value)
}

fn resolve_dynamic_number(
    state: &EvalState,
    material_def_names: &[String],
    material: MaterialKindRt,
    hit: RayHit,
    local_position: Vec3,
    view_dir: Vec3,
    function_name: &str,
) -> Option<f64> {
    let dynamic_material_id = match material {
        MaterialKindRt::Lambert(params)
        | MaterialKindRt::Metal(params)
        | MaterialKindRt::Dielectric(params) => params.dynamic_material_id,
    }?;
    let name = material_def_names.get(dynamic_material_id as usize)?;
    let ctx = make_shading_context(hit, local_position, view_dir);
    let value = eval_material_function(state, name, function_name, ctx).ok()?;
    let Value::Number(v) = value else {
        return None;
    };
    Some(v)
}

fn resolve_dynamic_normal(
    state: &EvalState,
    material_def_names: &[String],
    material: MaterialKindRt,
    hit: RayHit,
    local_position: Vec3,
    view_dir: Vec3,
    geometric_normal: Vec3,
) -> Option<Vec3> {
    let dynamic_material_id = match material {
        MaterialKindRt::Lambert(params)
        | MaterialKindRt::Metal(params)
        | MaterialKindRt::Dielectric(params) => params.dynamic_material_id,
    }?;
    let name = material_def_names.get(dynamic_material_id as usize)?;
    let ctx = make_shading_context_with_normal(hit, local_position, view_dir, geometric_normal);
    let value = eval_material_function(state, name, "normal", ctx).ok()?;
    let mut normal = vec3_from_value(&value)?.normalize();
    if normal.length() <= 1.0e-6 {
        return None;
    }
    if normal.dot(geometric_normal) < 0.0 {
        normal = normal.mul(-1.0);
    }
    Some(normal)
}

fn resolve_dynamic_object(
    state: &EvalState,
    material_def_names: &[String],
    material: MaterialKindRt,
    hit: RayHit,
    local_position: Vec3,
    view_dir: Vec3,
    function_name: &str,
) -> Option<Value> {
    let dynamic_material_id = match material {
        MaterialKindRt::Lambert(params)
        | MaterialKindRt::Metal(params)
        | MaterialKindRt::Dielectric(params) => params.dynamic_material_id,
    }?;
    let name = material_def_names.get(dynamic_material_id as usize)?;
    let ctx = make_shading_context(hit, local_position, view_dir);
    eval_material_function(state, name, function_name, ctx).ok()
}

fn resolve_pattern_color(params: MaterialParams, local_position: Vec3) -> Spectrum {
    match params.pattern {
        Some(ColorPattern::Checker3d {
            color_a,
            color_b,
            scale,
        }) => {
            let sx = (local_position.x * scale).floor() as i32;
            let sy = (local_position.y * scale).floor() as i32;
            let sz = (local_position.z * scale).floor() as i32;
            if ((sx + sy + sz) & 1) == 0 {
                color_a
            } else {
                color_b
            }
        }
        None => params.color,
    }
}

fn make_shading_context(hit: RayHit, local_position: Vec3, view_dir: Vec3) -> Value {
    let normal = if hit.front_face {
        hit.normal.normalize()
    } else {
        hit.normal.mul(-1.0).normalize()
    };
    make_shading_context_with_normal(hit, local_position, view_dir, normal)
}

fn make_shading_context_with_normal(
    hit: RayHit,
    local_position: Vec3,
    view_dir: Vec3,
    normal: Vec3,
) -> Value {
    let mut fields = std::collections::HashMap::new();
    fields.insert("position".to_string(), vec3_value_value(hit.position));
    fields.insert(
        "local_position".to_string(),
        vec3_value_value(local_position),
    );
    fields.insert("normal".to_string(), vec3_value_value(normal));
    fields.insert("view_dir".to_string(), vec3_value_value(view_dir));
    fields.insert(
        "front_face".to_string(),
        Value::Number(if hit.front_face { 1.0 } else { 0.0 }),
    );
    fields.insert("object_id".to_string(), Value::Number(hit.object_id as f64));
    fields.insert(
        "material_id".to_string(),
        Value::Number(hit.material_id as f64),
    );
    Value::Object(ObjectValue {
        type_name: None,
        fields,
    })
}

fn vec3_value_value(v: Vec3) -> Value {
    let mut fields = std::collections::HashMap::new();
    fields.insert("x".to_string(), Value::Number(v.x as f64));
    fields.insert("y".to_string(), Value::Number(v.y as f64));
    fields.insert("z".to_string(), Value::Number(v.z as f64));
    Value::Object(ObjectValue {
        type_name: Some("vec3".to_string()),
        fields,
    })
}

fn spectrum_from_value(value: &Value) -> Option<Spectrum> {
    match value {
        Value::Number(v) => Some(Spectrum::rgb(*v as f32, *v as f32, *v as f32)),
        Value::Object(obj) => {
            let r = read_number_field(obj, &["r", "x"])?;
            let g = read_number_field(obj, &["g", "y"])?;
            let b = read_number_field(obj, &["b", "z"])?;
            Some(Spectrum::rgb(r, g, b))
        }
    }
}

fn default_material() -> MaterialKindRt {
    MaterialKindRt::Lambert(MaterialParams::lambert(
        Spectrum::rgb(0.8, 0.8, 0.8),
        Spectrum::black(),
        0.0,
    ))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{
        Binding, DielectricMaterial, EvalState, LambertMaterial, MetalMaterial, Value,
        eval_program, parse_program,
    };

    use super::{
        AccelMode, RenderOptions, extract_scene_render_settings, render_depth_png_with_accel,
    };

    fn empty_state(bindings: HashMap<String, Binding>) -> EvalState {
        EvalState {
            bindings,
            function_defs: HashMap::new(),
            material_defs: HashMap::new(),
            sdf_defs: HashMap::new(),
        }
    }

    #[test]
    fn renders_depth_png_for_simple_sphere_scene() {
        let mut sphere_fields = HashMap::new();
        sphere_fields.insert("x".to_string(), Value::Number(0.0));
        sphere_fields.insert("y".to_string(), Value::Number(0.0));
        sphere_fields.insert("z".to_string(), Value::Number(0.0));

        let sphere = Value::Object(crate::ObjectValue {
            type_name: Some("Sphere".to_string()),
            fields: sphere_fields,
        });

        let mut bindings = HashMap::new();
        bindings.insert(
            "s".to_string(),
            Binding {
                mutable: true,
                value: sphere,
            },
        );
        let state = empty_state(bindings);

        let output = std::env::temp_dir().join("forgedthoughts-render-test.png");
        let _ = std::fs::remove_file(&output);
        render_depth_png_with_accel(
            &state,
            &output,
            RenderOptions {
                width: 64,
                height: 64,
                ..RenderOptions::default()
            },
            AccelMode::Naive,
        )
        .expect("render should succeed");

        let metadata = std::fs::metadata(&output).expect("output png should exist");
        assert!(metadata.len() > 0);
        let _ = std::fs::remove_file(output);
    }

    #[test]
    fn renders_non_black_for_offset_sphere() {
        let mut sphere_fields = HashMap::new();
        sphere_fields.insert("x".to_string(), Value::Number(10.0));
        sphere_fields.insert("y".to_string(), Value::Number(0.0));
        sphere_fields.insert("z".to_string(), Value::Number(0.0));

        let sphere = Value::Object(crate::ObjectValue {
            type_name: Some("Sphere".to_string()),
            fields: sphere_fields,
        });

        let mut bindings = HashMap::new();
        bindings.insert(
            "s".to_string(),
            Binding {
                mutable: true,
                value: sphere,
            },
        );
        let state = empty_state(bindings);

        let output = std::env::temp_dir().join("forgedthoughts-render-offset-test.png");
        let _ = std::fs::remove_file(&output);
        render_depth_png_with_accel(
            &state,
            &output,
            RenderOptions {
                width: 64,
                height: 64,
                ..RenderOptions::default()
            },
            AccelMode::Naive,
        )
        .expect("render should succeed");

        let image = image::open(&output).expect("png should load").into_luma8();
        let has_non_black = image.pixels().any(|pixel| pixel[0] > 0);
        assert!(has_non_black, "image should contain visible depth");
        let _ = std::fs::remove_file(output);
    }

    #[test]
    fn renders_with_all_accel_modes() {
        let mut sphere_fields = HashMap::new();
        sphere_fields.insert("x".to_string(), Value::Number(1.0));
        sphere_fields.insert("y".to_string(), Value::Number(0.0));
        sphere_fields.insert("z".to_string(), Value::Number(0.0));

        let sphere = Value::Object(crate::ObjectValue {
            type_name: Some("Sphere".to_string()),
            fields: sphere_fields,
        });
        let mut bindings = HashMap::new();
        bindings.insert(
            "s".to_string(),
            Binding {
                mutable: true,
                value: sphere,
            },
        );
        let state = empty_state(bindings);

        for mode in [AccelMode::Naive, AccelMode::Bvh, AccelMode::Bricks] {
            let output =
                std::env::temp_dir().join(format!("forgedthoughts-render-accel-{mode:?}.png"));
            let _ = std::fs::remove_file(&output);
            render_depth_png_with_accel(
                &state,
                &output,
                RenderOptions {
                    width: 32,
                    height: 32,
                    ..RenderOptions::default()
                },
                mode,
            )
            .expect("render should succeed");
            assert!(std::fs::metadata(&output).is_ok());
            let _ = std::fs::remove_file(output);
        }
    }

    #[test]
    fn uses_camera_and_lights_from_bindings() {
        let sphere = Value::Object(crate::ObjectValue {
            type_name: Some("Sphere".to_string()),
            fields: HashMap::new(),
        });

        let mut camera_fields = HashMap::new();
        camera_fields.insert("origin".to_string(), vec3_value(0.0, 0.0, 7.0));
        camera_fields.insert("target".to_string(), vec3_value(0.0, 0.0, 0.0));
        camera_fields.insert("fov_y".to_string(), Value::Number(40.0));
        let camera = Value::Object(crate::ObjectValue {
            type_name: Some("Camera".to_string()),
            fields: camera_fields,
        });

        let mut point_light_fields = HashMap::new();
        point_light_fields.insert("position".to_string(), vec3_value(2.0, 2.0, 4.0));
        point_light_fields.insert("intensity".to_string(), vec3_value(6.0, 6.0, 6.0));
        let point_light = Value::Object(crate::ObjectValue {
            type_name: Some("PointLight".to_string()),
            fields: point_light_fields,
        });

        let mut env_fields = HashMap::new();
        env_fields.insert("radiance".to_string(), vec3_value(0.08, 0.08, 0.08));
        let env_light = Value::Object(crate::ObjectValue {
            type_name: Some("EnvLight".to_string()),
            fields: env_fields,
        });

        let mut bindings = HashMap::new();
        bindings.insert(
            "s".to_string(),
            Binding {
                mutable: true,
                value: sphere,
            },
        );
        bindings.insert(
            "camera".to_string(),
            Binding {
                mutable: false,
                value: camera,
            },
        );
        bindings.insert(
            "light_key".to_string(),
            Binding {
                mutable: false,
                value: point_light,
            },
        );
        bindings.insert(
            "light_env".to_string(),
            Binding {
                mutable: false,
                value: env_light,
            },
        );

        let state = empty_state(bindings);
        let output = std::env::temp_dir().join("forgedthoughts-render-scene-config-test.png");
        let _ = std::fs::remove_file(&output);
        render_depth_png_with_accel(
            &state,
            &output,
            RenderOptions {
                width: 64,
                height: 64,
                ..RenderOptions::default()
            },
            AccelMode::Naive,
        )
        .expect("render with scene camera/lights should succeed");
        assert!(std::fs::metadata(&output).is_ok());
        let _ = std::fs::remove_file(output);
    }

    #[test]
    fn extracts_render_settings_from_bindings() {
        let mut render_fields = HashMap::new();
        render_fields.insert("width".to_string(), Value::Number(640.0));
        render_fields.insert("height".to_string(), Value::Number(360.0));
        render_fields.insert("max_steps".to_string(), Value::Number(180.0));
        render_fields.insert("max_dist".to_string(), Value::Number(60.0));
        render_fields.insert("epsilon".to_string(), Value::Number(0.0005));
        render_fields.insert("camera_z".to_string(), Value::Number(9.0));
        render_fields.insert("fov_y".to_string(), Value::Number(50.0));
        render_fields.insert("spp".to_string(), Value::Number(64.0));
        render_fields.insert("bounces".to_string(), Value::Number(8.0));
        render_fields.insert("min_spp".to_string(), Value::Number(12.0));
        render_fields.insert("noise_threshold".to_string(), Value::Number(0.025));
        render_fields.insert(
            "accel".to_string(),
            Value::Object(crate::ObjectValue {
                type_name: Some("Bvh".to_string()),
                fields: HashMap::new(),
            }),
        );
        let render = Value::Object(crate::ObjectValue {
            type_name: Some("RenderSettings".to_string()),
            fields: render_fields,
        });
        let mut bindings = HashMap::new();
        bindings.insert(
            "render".to_string(),
            Binding {
                mutable: false,
                value: render,
            },
        );
        let state = empty_state(bindings);

        let settings = extract_scene_render_settings(&state);
        assert_eq!(settings.width, Some(640));
        assert_eq!(settings.height, Some(360));
        assert_eq!(settings.max_steps, Some(180));
        assert_eq!(settings.max_dist, Some(60.0));
        assert_eq!(settings.epsilon, Some(0.0005));
        assert_eq!(settings.camera_z, Some(9.0));
        assert_eq!(settings.fov_y_degrees, Some(50.0));
        assert_eq!(settings.trace_spp, Some(64));
        assert_eq!(settings.trace_bounces, Some(8));
        assert_eq!(settings.trace_min_spp, Some(12));
        assert_eq!(settings.trace_noise_threshold, Some(0.025));
        assert_eq!(settings.accel, Some(AccelMode::Bvh));
    }

    #[test]
    fn extracts_legacy_material_as_dielectric_from_scene_object() {
        let mut material_fields = HashMap::new();
        material_fields.insert("roughness".to_string(), Value::Number(0.2));
        material_fields.insert("metallic".to_string(), Value::Number(0.7));
        material_fields.insert("specular".to_string(), Value::Number(0.9));
        material_fields.insert("specular_weight".to_string(), Value::Number(0.8));
        material_fields.insert("specular_color".to_string(), vec3_value(0.7, 0.8, 0.9));
        material_fields.insert("ior".to_string(), Value::Number(1.65));
        material_fields.insert("clearcoat".to_string(), Value::Number(0.3));
        material_fields.insert("clearcoat_roughness".to_string(), Value::Number(0.12));
        material_fields.insert("transmission".to_string(), Value::Number(0.55));
        material_fields.insert("thin_walled".to_string(), Value::Number(1.0));
        material_fields.insert("anisotropy".to_string(), Value::Number(-0.25));
        material_fields.insert("anisotropy_rotation".to_string(), Value::Number(0.33));
        material_fields.insert("emission_color".to_string(), vec3_value(0.1, 0.2, 0.3));
        material_fields.insert("emission_strength".to_string(), Value::Number(3.0));
        material_fields.insert("base_color".to_string(), vec3_value(0.9, 0.5, 0.2));
        let material = Value::Object(crate::ObjectValue {
            type_name: Some("Material".to_string()),
            fields: material_fields,
        });

        let mut sphere_fields = HashMap::new();
        sphere_fields.insert("material".to_string(), material);
        let sphere = Value::Object(crate::ObjectValue {
            type_name: Some("Sphere".to_string()),
            fields: sphere_fields,
        });

        let mut bindings = HashMap::new();
        bindings.insert(
            "s".to_string(),
            Binding {
                mutable: true,
                value: sphere.clone(),
            },
        );
        let state = empty_state(bindings);

        let mat = super::parse_material(&state, &sphere);
        let super::MaterialKindRt::Dielectric(mat) = mat else {
            panic!("expected Dielectric material");
        };
        assert!((mat.roughness - 0.2).abs() < 1.0e-6);
        assert!((mat.ior - 1.65).abs() < 1.0e-6);
        assert!(mat.thin_walled);
        assert!((mat.emission_color.g - 0.2).abs() < 1.0e-6);
        assert!((mat.emission_strength - 3.0).abs() < 1.0e-6);
        assert!((mat.color.r - 0.9).abs() < 1.0e-6);
    }

    #[test]
    fn parses_lambert_material_constructor() {
        let mut material_fields = HashMap::new();
        material_fields.insert("color".to_string(), vec3_value(0.6, 0.4, 0.2));
        material_fields.insert("emission_color".to_string(), vec3_value(0.0, 0.1, 0.0));
        material_fields.insert("emission_strength".to_string(), Value::Number(2.0));
        let material = Value::Object(crate::ObjectValue {
            type_name: Some("Lambert".to_string()),
            fields: material_fields,
        });

        let mut sphere_fields = HashMap::new();
        sphere_fields.insert("material".to_string(), material);
        let sphere = Value::Object(crate::ObjectValue {
            type_name: Some("Sphere".to_string()),
            fields: sphere_fields,
        });

        let state = empty_state(HashMap::new());
        let mat = super::parse_material(&state, &sphere);
        let super::MaterialKindRt::Lambert(LambertMaterial {
            color,
            emission_color,
            emission_strength,
            ..
        }) = mat
        else {
            panic!("expected Lambert material");
        };
        assert!((color.r - 0.6).abs() < 1.0e-6);
        assert!((emission_color.g - 0.1).abs() < 1.0e-6);
        assert!((emission_strength - 2.0).abs() < 1.0e-6);
    }

    #[test]
    fn parses_metal_material_constructor() {
        let mut material_fields = HashMap::new();
        material_fields.insert("color".to_string(), vec3_value(0.8, 0.7, 0.6));
        material_fields.insert("roughness".to_string(), Value::Number(0.35));
        let material = Value::Object(crate::ObjectValue {
            type_name: Some("Metal".to_string()),
            fields: material_fields,
        });

        let mut sphere_fields = HashMap::new();
        sphere_fields.insert("material".to_string(), material);
        let sphere = Value::Object(crate::ObjectValue {
            type_name: Some("Sphere".to_string()),
            fields: sphere_fields,
        });

        let state = empty_state(HashMap::new());
        let mat = super::parse_material(&state, &sphere);
        let super::MaterialKindRt::Metal(MetalMaterial {
            color, roughness, ..
        }) = mat
        else {
            panic!("expected Metal material");
        };
        assert!((color.r - 0.8).abs() < 1.0e-6);
        assert!((roughness - 0.35).abs() < 1.0e-6);
    }

    #[test]
    fn parses_dielectric_material_constructor() {
        let mut material_fields = HashMap::new();
        material_fields.insert("color".to_string(), vec3_value(0.95, 0.97, 1.0));
        material_fields.insert("ior".to_string(), Value::Number(1.52));
        material_fields.insert("roughness".to_string(), Value::Number(0.02));
        material_fields.insert("thin_walled".to_string(), Value::Number(1.0));
        let material = Value::Object(crate::ObjectValue {
            type_name: Some("Dielectric".to_string()),
            fields: material_fields,
        });

        let mut sphere_fields = HashMap::new();
        sphere_fields.insert("material".to_string(), material);
        let sphere = Value::Object(crate::ObjectValue {
            type_name: Some("Sphere".to_string()),
            fields: sphere_fields,
        });

        let state = empty_state(HashMap::new());
        let mat = super::parse_material(&state, &sphere);
        let super::MaterialKindRt::Dielectric(DielectricMaterial {
            color,
            ior,
            roughness,
            thin_walled,
            ..
        }) = mat
        else {
            panic!("expected Dielectric material");
        };
        assert!((color.r - 0.95).abs() < 1.0e-6);
        assert!((ior - 1.52).abs() < 1.0e-6);
        assert!((roughness - 0.02).abs() < 1.0e-6);
        assert!(thin_walled);
    }

    #[test]
    fn parses_material_fields_computed_from_ft_expressions() {
        let source = r#"
            let tint = mix(vec3(0.9, 0.7, 0.25), vec3(1.0), 0.2);
            let polish = clamp(0.08 + 0.04 * 2.0, 0.0, 1.0);
            let glow = max(0.5, 0.2 * 4.0);

            let material = Metal {
              color: tint,
              roughness: polish,
              emission_color: vec3(glow * 0.1, glow * 0.08, glow * 0.04),
              emission_strength: glow - 0.5
            };

            let scene = Sphere {
              material: material
            };
        "#;

        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");
        let scene = &state.bindings.get("scene").expect("scene binding").value;

        let mat = super::parse_material(&state, scene);
        let super::MaterialKindRt::Metal(MetalMaterial {
            color,
            roughness,
            emission_color,
            emission_strength,
            ..
        }) = mat
        else {
            panic!("expected Metal material");
        };

        assert!((color.r - 0.92).abs() < 1.0e-6);
        assert!((color.g - 0.76).abs() < 1.0e-6);
        assert!((color.b - 0.4).abs() < 1.0e-6);
        assert!((roughness - 0.16).abs() < 1.0e-6);
        assert!((emission_color.r - 0.08).abs() < 1.0e-6);
        assert!((emission_color.g - 0.064).abs() < 1.0e-6);
        assert!((emission_color.b - 0.032).abs() < 1.0e-6);
        assert!((emission_strength - 0.3).abs() < 1.0e-6);
    }

    #[test]
    fn resolves_checker_pattern_from_local_hit_position() {
        let source = r#"
            let material = Lambert {
              color: vec3(0.5),
              pattern: Checker3d {
                color_a: vec3(0.1, 0.2, 0.3),
                color_b: vec3(0.9, 0.8, 0.7),
                scale: 2.0
              }
            };

            let scene = Sphere {
              material: material
            };
        "#;

        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");
        let scene = super::compile_scene(
            &state,
            state
                .bindings
                .get("scene")
                .map(|b| &b.value)
                .expect("scene binding"),
            super::default_material(),
        )
        .expect("scene should compile");
        let setup = super::build_render_setup(&state, &scene, RenderOptions::default());

        let hit_a = super::RayHit {
            t: 1.0,
            position: super::Vec3::new(0.1, 0.1, 0.1),
            normal: super::Vec3::new(0.0, 1.0, 0.0),
            front_face: true,
            object_id: 1,
            material_id: 1,
        };
        let hit_b = super::RayHit {
            t: 1.0,
            position: super::Vec3::new(0.6, 0.1, 0.1),
            ..hit_a
        };

        let super::MaterialKindRt::Lambert(mat_a) =
            super::resolve_material_at_hit(&setup, hit_a, super::Vec3::new(0.0, 0.0, 1.0))
        else {
            panic!("expected Lambert material");
        };
        let super::MaterialKindRt::Lambert(mat_b) =
            super::resolve_material_at_hit(&setup, hit_b, super::Vec3::new(0.0, 0.0, 1.0))
        else {
            panic!("expected Lambert material");
        };

        assert!((mat_a.color.r - 0.1).abs() < 1.0e-6);
        assert!((mat_a.color.g - 0.2).abs() < 1.0e-6);
        assert!((mat_b.color.r - 0.9).abs() < 1.0e-6);
        assert!((mat_b.color.g - 0.8).abs() < 1.0e-6);
    }

    #[test]
    fn resolves_ft_material_color_from_local_hit_position() {
        let source = r#"
            material Stripe {
              model: Lambert;
              let warm = vec3(0.9, 0.72, 0.3);
              let dark = vec3(0.12, 0.14, 0.18);
              fn color(ctx) {
                let phase = sin(ctx.local_position.x * 8.0);
                let mask = step(0.0, phase);
                return mix(warm, dark, mask);
              }
            };

            let scene = Sphere {
              material: Stripe {}
            };
        "#;

        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");
        let scene = super::compile_scene(
            &state,
            state
                .bindings
                .get("scene")
                .map(|b| &b.value)
                .expect("scene binding"),
            super::default_material(),
        )
        .expect("scene should compile");
        let setup = super::build_render_setup(&state, &scene, RenderOptions::default());

        let hit_warm = super::RayHit {
            t: 1.0,
            position: super::Vec3::new(-0.1, 0.0, 0.0),
            normal: super::Vec3::new(0.0, 1.0, 0.0),
            front_face: true,
            object_id: 1,
            material_id: 1,
        };
        let hit_dark = super::RayHit {
            t: 1.0,
            position: super::Vec3::new(0.1, 0.0, 0.0),
            ..hit_warm
        };

        let super::MaterialKindRt::Lambert(mat_warm) =
            super::resolve_material_at_hit(&setup, hit_warm, super::Vec3::new(0.0, 0.0, 1.0))
        else {
            panic!("expected Lambert material");
        };
        let super::MaterialKindRt::Lambert(mat_dark) =
            super::resolve_material_at_hit(&setup, hit_dark, super::Vec3::new(0.0, 0.0, 1.0))
        else {
            panic!("expected Lambert material");
        };

        assert!((mat_warm.color.r - 0.9).abs() < 1.0e-6);
        assert!((mat_warm.color.g - 0.72).abs() < 1.0e-6);
        assert!((mat_dark.color.r - 0.12).abs() < 1.0e-6);
        assert!((mat_dark.color.g - 0.14).abs() < 1.0e-6);
    }

    #[test]
    fn resolves_ft_material_static_color_property() {
        let source = r#"
            material Solid {
              model: Lambert;
              let tint = vec3(0.24, 0.5, 0.82);
              color = tint;
            };

            let scene = Sphere {
              material: Solid {}
            };
        "#;

        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");
        let scene = &state.bindings.get("scene").expect("scene binding").value;
        let super::MaterialKindRt::Lambert(mat) = super::parse_material(&state, scene) else {
            panic!("expected Lambert material");
        };

        assert!((mat.color.r - 0.24).abs() < 1.0e-6);
        assert!((mat.color.g - 0.5).abs() < 1.0e-6);
        assert!((mat.color.b - 0.82).abs() < 1.0e-6);
    }

    #[test]
    fn resolves_ft_material_dynamic_roughness_hook() {
        let source = r#"
            material RoughStripe {
              model: Metal;
              color = vec3(0.8, 0.72, 0.3);
              roughness = 0.15;
              fn roughness(ctx) {
                let phase = sin(ctx.local_position.x * 6.0);
                let mask = step(0.0, phase);
                return mix(0.12, 0.48, mask);
              }
            };

            let scene = Sphere {
              material: RoughStripe {}
            };
        "#;

        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");
        let scene = super::compile_scene(
            &state,
            state
                .bindings
                .get("scene")
                .map(|b| &b.value)
                .expect("scene binding"),
            super::default_material(),
        )
        .expect("scene should compile");
        let setup = super::build_render_setup(&state, &scene, RenderOptions::default());

        let hit_smooth = super::RayHit {
            t: 1.0,
            position: super::Vec3::new(-0.1, 0.0, 0.0),
            normal: super::Vec3::new(0.0, 1.0, 0.0),
            front_face: true,
            object_id: 1,
            material_id: 1,
        };
        let hit_rough = super::RayHit {
            t: 1.0,
            position: super::Vec3::new(0.1, 0.0, 0.0),
            ..hit_smooth
        };

        let super::MaterialKindRt::Metal(mat_smooth) =
            super::resolve_material_at_hit(&setup, hit_smooth, super::Vec3::new(0.0, 0.0, 1.0))
        else {
            panic!("expected Metal material");
        };
        let super::MaterialKindRt::Metal(mat_rough) =
            super::resolve_material_at_hit(&setup, hit_rough, super::Vec3::new(0.0, 0.0, 1.0))
        else {
            panic!("expected Metal material");
        };

        assert!((mat_smooth.roughness - 0.12).abs() < 1.0e-6);
        assert!((mat_rough.roughness - 0.48).abs() < 1.0e-6);
        assert!((mat_smooth.color.r - 0.8).abs() < 1.0e-6);
    }

    #[test]
    fn resolves_ft_material_dynamic_dielectric_hooks() {
        let source = r#"
            material RippleGlass {
              model: Dielectric;
              color = vec3(1.0);
              ior = 1.52;
              roughness = 0.02;
              thin_walled = 0.0;
              fn ior(ctx) {
                let phase = sin(ctx.local_position.x * 6.0);
                let mask = step(0.0, phase);
                return mix(1.45, 1.6, mask);
              }
              fn thin_walled(ctx) {
                let phase = sin(ctx.local_position.y * 6.0);
                return step(0.0, phase);
              }
            };

            let scene = Sphere {
              material: RippleGlass {}
            };
        "#;

        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");
        let scene = super::compile_scene(
            &state,
            state
                .bindings
                .get("scene")
                .map(|b| &b.value)
                .expect("scene binding"),
            super::default_material(),
        )
        .expect("scene should compile");
        let setup = super::build_render_setup(&state, &scene, RenderOptions::default());

        let hit_solid = super::RayHit {
            t: 1.0,
            position: super::Vec3::new(-0.1, -0.1, 0.0),
            normal: super::Vec3::new(0.0, 1.0, 0.0),
            front_face: true,
            object_id: 1,
            material_id: 1,
        };
        let hit_thin = super::RayHit {
            t: 1.0,
            position: super::Vec3::new(0.1, 0.1, 0.0),
            ..hit_solid
        };

        let super::MaterialKindRt::Dielectric(mat_solid) =
            super::resolve_material_at_hit(&setup, hit_solid, super::Vec3::new(0.0, 0.0, 1.0))
        else {
            panic!("expected Dielectric material");
        };
        let super::MaterialKindRt::Dielectric(mat_thin) =
            super::resolve_material_at_hit(&setup, hit_thin, super::Vec3::new(0.0, 0.0, 1.0))
        else {
            panic!("expected Dielectric material");
        };

        assert!((mat_solid.ior - 1.45).abs() < 1.0e-6);
        assert!((mat_thin.ior - 1.6).abs() < 1.0e-6);
        assert!(!mat_solid.thin_walled);
        assert!(mat_thin.thin_walled);
    }

    #[test]
    fn parses_medium_and_subsurface_material_fields() {
        let source = r#"
            let scene = Sphere {
              material: Dielectric {
                color: vec3(0.96, 0.98, 1.0),
                ior: 1.52,
                medium: Medium {
                  ior: 1.33,
                  absorption_color: vec3(0.82, 0.91, 1.0),
                  density: 0.35
                },
                subsurface: Subsurface {
                  color: vec3(1.0, 0.58, 0.44),
                  radius: vec3(0.8, 0.4, 0.22),
                  anisotropy: 0.15,
                  scale: 1.25
                }
              }
            };
        "#;

        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");
        let scene = &state.bindings.get("scene").expect("scene binding").value;
        let super::MaterialKindRt::Dielectric(mat) = super::parse_material(&state, scene) else {
            panic!("expected Dielectric material");
        };

        let medium = mat.medium.expect("expected medium params");
        assert!((medium.ior - 1.33).abs() < 1.0e-6);
        assert!((medium.absorption_color.b - 1.0).abs() < 1.0e-6);
        assert!((medium.density - 0.35).abs() < 1.0e-6);

        let subsurface = mat.subsurface.expect("expected subsurface params");
        assert!((subsurface.color.g - 0.58).abs() < 1.0e-6);
        assert!((subsurface.radius.x - 0.8).abs() < 1.0e-6);
        assert!((subsurface.radius.z - 0.22).abs() < 1.0e-6);
        assert!((subsurface.anisotropy - 0.15).abs() < 1.0e-6);
        assert!((subsurface.scale - 1.25).abs() < 1.0e-6);
    }

    #[test]
    fn resolves_ft_material_dynamic_medium_and_subsurface_hooks() {
        let source = r#"
            material WaxedGlass {
              model: Dielectric;
              color = vec3(1.0);
              medium = Medium {
                ior: 1.33,
                absorption_color: vec3(0.9, 0.95, 1.0),
                density: 0.05
              };
              subsurface = Subsurface {
                color: vec3(1.0, 0.6, 0.45),
                radius: vec3(0.5),
                anisotropy: 0.0,
                scale: 0.0
              };
              fn medium(ctx) {
                let phase = sin(ctx.local_position.x * 6.0);
                let mask = step(0.0, phase);
                return Medium {
                  ior: mix(1.33, 1.45, mask),
                  absorption_color: vec3(0.84, 0.92, 1.0),
                  density: mix(0.1, 0.45, mask)
                };
              }
              fn subsurface(ctx) {
                let phase = sin(ctx.local_position.y * 6.0);
                let mask = step(0.0, phase);
                return Subsurface {
                  color: mix(vec3(1.0, 0.62, 0.48), vec3(0.92, 0.42, 0.32), mask),
                  radius: mix(vec3(0.35), vec3(0.9, 0.5, 0.22), mask),
                  anisotropy: mix(0.0, 0.25, mask),
                  scale: mix(0.2, 1.1, mask)
                };
              }
            };

            let scene = Sphere {
              material: WaxedGlass {}
            };
        "#;

        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");
        let scene = super::compile_scene(
            &state,
            state
                .bindings
                .get("scene")
                .map(|b| &b.value)
                .expect("scene binding"),
            super::default_material(),
        )
        .expect("scene should compile");
        let setup = super::build_render_setup(&state, &scene, RenderOptions::default());

        let hit_a = super::RayHit {
            t: 1.0,
            position: super::Vec3::new(-0.1, -0.1, 0.0),
            normal: super::Vec3::new(0.0, 1.0, 0.0),
            front_face: true,
            object_id: 1,
            material_id: 1,
        };
        let hit_b = super::RayHit {
            t: 1.0,
            position: super::Vec3::new(0.1, 0.1, 0.0),
            ..hit_a
        };

        let super::MaterialKindRt::Dielectric(mat_a) =
            super::resolve_material_at_hit(&setup, hit_a, super::Vec3::new(0.0, 0.0, 1.0))
        else {
            panic!("expected Dielectric material");
        };
        let super::MaterialKindRt::Dielectric(mat_b) =
            super::resolve_material_at_hit(&setup, hit_b, super::Vec3::new(0.0, 0.0, 1.0))
        else {
            panic!("expected Dielectric material");
        };

        let medium_a = mat_a.medium.expect("expected medium params");
        let medium_b = mat_b.medium.expect("expected medium params");
        assert!((medium_a.ior - 1.33).abs() < 1.0e-6);
        assert!((medium_b.ior - 1.45).abs() < 1.0e-6);
        assert!((medium_a.density - 0.1).abs() < 1.0e-6);
        assert!((medium_b.density - 0.45).abs() < 1.0e-6);

        let subsurface_a = mat_a.subsurface.expect("expected subsurface params");
        let subsurface_b = mat_b.subsurface.expect("expected subsurface params");
        assert!((subsurface_a.scale - 0.2).abs() < 1.0e-6);
        assert!((subsurface_b.scale - 1.1).abs() < 1.0e-6);
        assert!((subsurface_a.radius.x - 0.35).abs() < 1.0e-6);
        assert!((subsurface_b.radius.x - 0.9).abs() < 1.0e-6);
        assert!((subsurface_b.anisotropy - 0.25).abs() < 1.0e-6);
    }

    #[test]
    fn applies_medium_transmittance_per_segment() {
        let medium = super::MediumState {
            ior: 1.33,
            absorption_color: crate::Spectrum::rgb(0.5, 0.8, 1.0),
            density: 2.0,
        };
        let trans = super::medium_transmittance(medium, 1.5);
        assert!((trans.r - 0.125).abs() < 1.0e-6);
        assert!((trans.g - 0.512).abs() < 1.0e-6);
        assert!((trans.b - 1.0).abs() < 1.0e-6);
    }

    #[test]
    fn transitions_medium_when_crossing_dielectric() {
        let mut params = crate::MaterialParams::dielectric(
            crate::Spectrum::rgb(1.0, 1.0, 1.0),
            1.52,
            0.0,
            false,
            crate::Spectrum::black(),
            0.0,
        );
        params.medium = Some(crate::MediumParams::new(
            1.33,
            crate::Spectrum::rgb(0.9, 0.8, 0.7),
            0.25,
        ));
        let material = super::MaterialKindRt::Dielectric(params);

        let entered = super::transition_medium(material, true, super::MediumState::air());
        assert!((entered.ior - 1.33).abs() < 1.0e-6);
        assert!((entered.absorption_color.g - 0.8).abs() < 1.0e-6);
        assert!((entered.density - 0.25).abs() < 1.0e-6);

        let exited = super::transition_medium(material, false, entered);
        assert!((exited.ior - 1.0).abs() < 1.0e-6);
        assert!((exited.absorption_color.r - 1.0).abs() < 1.0e-6);
        assert!(exited.density.abs() < 1.0e-6);
    }

    #[test]
    fn resolves_ft_custom_bsdf_hooks() {
        let source = r#"
            material CustomBsdf {
              model: Lambert;
              color = vec3(0.2);
              fn eval(ctx) {
                return vec3(0.25, 0.5, 0.75);
              }
              fn pdf(ctx) {
                return 0.42;
              }
              fn sample(ctx) {
                return BsdfSample {
                  wi: ctx.normal,
                  f: vec3(0.6, 0.4, 0.2),
                  pdf: 0.35,
                  delta: 0.0,
                  apply_cos: 1.0,
                  transmission: 0.0,
                  thin_walled: 0.0,
                  next_ior: ctx.current_ior
                };
              }
            };

            let scene = Sphere {
              material: CustomBsdf {}
            };
        "#;

        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");
        let scene = super::compile_scene(
            &state,
            state
                .bindings
                .get("scene")
                .map(|b| &b.value)
                .expect("scene binding"),
            super::default_material(),
        )
        .expect("scene should compile");
        let setup = super::build_render_setup(&state, &scene, RenderOptions::default());
        let hit = super::RayHit {
            t: 1.0,
            position: super::Vec3::new(0.0, 0.0, 1.0),
            normal: super::Vec3::new(0.0, 0.0, 1.0),
            front_face: true,
            object_id: 1,
            material_id: 1,
        };
        let mat = super::resolve_material_at_hit(&setup, hit, super::Vec3::new(0.0, 0.0, 1.0));
        let bsdf_ctx = super::BsdfContextBase {
            hit,
            local_position: super::Vec3::new(0.0, 0.0, 1.0),
            normal: super::Vec3::new(0.0, 0.0, 1.0),
            wo: super::Vec3::new(0.0, 0.0, 1.0),
            current_ior: 1.1,
        };

        let eval = super::eval_bsdf(&setup, mat, bsdf_ctx, super::Vec3::new(0.0, 0.0, 1.0));
        let pdf = super::pdf_bsdf(&setup, mat, bsdf_ctx, super::Vec3::new(0.0, 0.0, 1.0));
        let mut rng = super::XorShift64::new(1234);
        let sample = super::sample_bsdf_lobe(&setup, mat, bsdf_ctx, &mut rng);

        assert!((eval.r - 0.25).abs() < 1.0e-6);
        assert!((eval.g - 0.5).abs() < 1.0e-6);
        assert!((eval.b - 0.75).abs() < 1.0e-6);
        assert!((pdf - 0.42).abs() < 1.0e-6);
        assert!((sample.wi.z - 1.0).abs() < 1.0e-6);
        assert!((sample.f.r - 0.6).abs() < 1.0e-6);
        assert!((sample.f.g - 0.4).abs() < 1.0e-6);
        assert!((sample.pdf - 0.35).abs() < 1.0e-6);
        assert!((sample.next_ior - 1.1).abs() < 1.0e-6);
    }

    #[test]
    fn resolves_ft_material_dynamic_normal_hook() {
        let source = r#"
            material Bumped {
              model: Lambert;
              fn normal(ctx) {
                return normalize(vec3(ctx.normal.x + 0.35, ctx.normal.y, ctx.normal.z));
              }
            };

            let scene = Sphere {
              material: Bumped {}
            };
        "#;

        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");
        let scene = super::compile_scene(
            &state,
            state
                .bindings
                .get("scene")
                .map(|b| &b.value)
                .expect("scene binding"),
            super::default_material(),
        )
        .expect("scene should compile");
        let setup = super::build_render_setup(&state, &scene, RenderOptions::default());
        let hit = super::RayHit {
            t: 1.0,
            position: super::Vec3::new(0.0, 0.0, 1.0),
            normal: super::Vec3::new(0.0, 0.0, 1.0),
            front_face: true,
            object_id: 1,
            material_id: 1,
        };

        let ctx = super::build_bsdf_context(&setup, hit, super::Vec3::new(0.0, 0.0, 1.0), 1.0);
        assert!(ctx.normal.x > 0.2);
        assert!(ctx.normal.z > 0.9);
    }

    #[test]
    fn resolves_material_local_helper_functions_across_hooks() {
        let source = r#"
            material Brick {
              model: Lambert;
              let brick = vec3(0.68, 0.24, 0.16);
              let mortar = vec3(0.8, 0.77, 0.72);

              fn mortar_mask(ctx) {
                let band_x = smoothstep(-0.12, 0.12, sin(ctx.local_position.x * 11.0));
                let band_y = smoothstep(-0.12, 0.12, sin(ctx.local_position.y * 7.0));
                return max(band_x, band_y);
              }

              fn color(ctx) {
                return mix(brick, mortar, mortar_mask(ctx));
              }

              fn roughness(ctx) {
                return mix(0.7, 0.95, mortar_mask(ctx));
              }

              fn normal(ctx) {
                let m = mortar_mask(ctx);
                return normalize(ctx.normal + vec3(m * 0.2, 0.0, 0.0));
              }
            };

            let scene = Sphere {
              material: Brick {}
            };
        "#;

        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");
        let scene = super::compile_scene(
            &state,
            state
                .bindings
                .get("scene")
                .map(|b| &b.value)
                .expect("scene binding"),
            super::default_material(),
        )
        .expect("scene should compile");
        let setup = super::build_render_setup(&state, &scene, RenderOptions::default());
        let hit = super::RayHit {
            t: 1.0,
            position: super::Vec3::new(0.1, 0.1, 1.0),
            normal: super::Vec3::new(0.0, 0.0, 1.0),
            front_face: true,
            object_id: 1,
            material_id: 1,
        };

        let super::MaterialKindRt::Lambert(mat) =
            super::resolve_material_at_hit(&setup, hit, super::Vec3::new(0.0, 0.0, 1.0))
        else {
            panic!("expected Lambert material");
        };
        let ctx = super::build_bsdf_context(&setup, hit, super::Vec3::new(0.0, 0.0, 1.0), 1.0);

        assert!(mat.color.r > 0.7);
        assert!(mat.roughness > 0.8);
        assert!(ctx.normal.x > 0.05);
    }

    #[test]
    fn renders_cylinder_and_torus_with_rotation() {
        let mut cyl_fields = HashMap::new();
        cyl_fields.insert("radius".to_string(), Value::Number(0.7));
        cyl_fields.insert("height".to_string(), Value::Number(2.2));
        let mut rot_fields = HashMap::new();
        rot_fields.insert("x".to_string(), Value::Number(20.0));
        rot_fields.insert("z".to_string(), Value::Number(15.0));
        cyl_fields.insert(
            "rot".to_string(),
            Value::Object(crate::ObjectValue {
                type_name: None,
                fields: rot_fields,
            }),
        );
        let cylinder = Value::Object(crate::ObjectValue {
            type_name: Some("Cylinder".to_string()),
            fields: cyl_fields,
        });

        let mut torus_fields = HashMap::new();
        torus_fields.insert("major_radius".to_string(), Value::Number(1.6));
        torus_fields.insert("minor_radius".to_string(), Value::Number(0.25));
        torus_fields.insert("y".to_string(), Value::Number(0.4));
        let torus = Value::Object(crate::ObjectValue {
            type_name: Some("Torus".to_string()),
            fields: torus_fields,
        });

        let mut add_fields = HashMap::new();
        add_fields.insert("lhs".to_string(), cylinder);
        add_fields.insert("rhs".to_string(), torus);
        let scene = Value::Object(crate::ObjectValue {
            type_name: Some("add".to_string()),
            fields: add_fields,
        });

        let mut bindings = HashMap::new();
        bindings.insert(
            "scene".to_string(),
            Binding {
                mutable: false,
                value: scene,
            },
        );
        let state = empty_state(bindings);

        let output = std::env::temp_dir().join("forgedthoughts-render-new-primitives-test.png");
        let _ = std::fs::remove_file(&output);
        render_depth_png_with_accel(
            &state,
            &output,
            RenderOptions {
                width: 96,
                height: 96,
                ..RenderOptions::default()
            },
            AccelMode::Naive,
        )
        .expect("render should succeed");
        assert!(std::fs::metadata(&output).is_ok());
        let _ = std::fs::remove_file(output);
    }

    #[test]
    fn compiles_and_evaluates_custom_ft_sdf() {
        let source = r#"
            sdf SoftBlob {
              let wave_scale = 0.15;

              fn bounds() {
                return vec3(1.2, 1.2, 1.0);
              }

              fn warp(p) {
                return vec3(p.x, p.y + sin(p.x * 4.0) * wave_scale, p.z);
              }

              fn distance(p) {
                let q = warp(p);
                return length(q) - 1.0;
              }
            };

            let scene = SoftBlob {
              material: Lambert {
                color: vec3(0.7, 0.45, 0.3)
              }
            };
        "#;

        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");
        let scene = super::compile_scene(
            &state,
            state
                .bindings
                .get("scene")
                .map(|b| &b.value)
                .expect("scene binding"),
            super::default_material(),
        )
        .expect("scene should compile");

        let center = super::sdf_distance_info(&scene.root, super::Vec3::new(0.0, 0.0, 0.0));
        let surface = super::sdf_distance_info(&scene.root, super::Vec3::new(0.0, 1.0, 0.0));
        let bounds = super::sdf_bounds(&scene.root);

        assert!(center.distance < -0.9);
        assert!(surface.distance.abs() < 1.0e-4);
        assert!(bounds.max.x < 3.0);
    }

    fn vec3_value(x: f64, y: f64, z: f64) -> Value {
        let mut fields = HashMap::new();
        fields.insert("x".to_string(), Value::Number(x));
        fields.insert("y".to_string(), Value::Number(y));
        fields.insert("z".to_string(), Value::Number(z));
        Value::Object(crate::ObjectValue {
            type_name: Some("vec3".to_string()),
            fields,
        })
    }
}
