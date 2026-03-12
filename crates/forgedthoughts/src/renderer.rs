use std::collections::VecDeque;
use std::path::Path;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use image::{ImageError, RgbImage};
use rayon::prelude::*;
use thiserror::Error;

use crate::{
    BlendedMaterial, ColorPattern, EvalState, FunctionValue, Material, MaterialKindTag,
    MaterialParams, MaterialSampleInput, MediumParams, ObjectValue, SubsurfaceParams, Value,
    eval_environment_function, eval_function_value, eval_material_function_with_overrides,
    eval_material_properties_with_overrides, eval_sdf_function_args_with_overrides,
    eval_sdf_function_with_overrides, eval_sdf_vec3_function_with_overrides,
    eval_sdf_zero_arg_function_with_overrides,
    render_api::{
        Camera, CameraKind, EnvLight, Light, PinholeCamera, PointLight, Spectrum, SphereLight,
        Vec3 as ApiVec3,
    },
};

#[allow(dead_code)]
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
    pub step_scale: f32,
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
            step_scale: 0.7,
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
    pub step_scale: Option<f32>,
    pub camera_z: Option<f32>,
    pub fov_y_degrees: Option<f32>,
    pub accel: Option<AccelMode>,
    pub trace_spp: Option<u32>,
    pub trace_bounces: Option<u32>,
    pub trace_min_spp: Option<u32>,
    pub trace_noise_threshold: Option<f32>,
}

#[allow(dead_code)]
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

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct PathtraceSettings {
    pub spp: u32,
    pub max_bounces: u32,
    pub preview_every: u32,
    pub min_spp: u32,
    pub noise_threshold: f32,
}

#[allow(dead_code)]
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
    pub aa_samples: u32,
    pub debug_aov: Option<RayDebugAov>,
}

impl Default for RaySettings {
    fn default() -> Self {
        Self {
            max_depth: 8,
            tile_size: 64,
            aa_samples: 1,
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
        shell: f32,
        object_id: u32,
        material_id: u32,
    },
    Box {
        transform: PrimitiveTransform,
        half_size: Vec3,
        round: f32,
        shell: f32,
        object_id: u32,
        material_id: u32,
    },
    Cylinder {
        transform: PrimitiveTransform,
        radius: f32,
        half_height: f32,
        round: f32,
        shell: f32,
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
    ExtrudePolygon {
        transform: PrimitiveTransform,
        sides: u32,
        radius: f32,
        half_height: f32,
        round: f32,
        shell: f32,
        object_id: u32,
        material_id: u32,
    },
    Custom {
        transform: PrimitiveTransform,
        runtime: Arc<CustomSdfRuntime>,
        bounds_half_extents: Vec3,
        object_id: u32,
        material_id: u32,
    },
    DomainModifier {
        base: Box<SdfNode>,
        runtime: Arc<ModifierFunctionRuntime>,
        transform: PrimitiveTransform,
        bounds: Aabb,
    },
    DistancePostModifier {
        base: Box<SdfNode>,
        runtime: Arc<ModifierFunctionRuntime>,
        transform: PrimitiveTransform,
        bounds: Aabb,
    },
    Union {
        lhs: Box<SdfNode>,
        rhs: Box<SdfNode>,
    },
    Intersect {
        lhs: Box<SdfNode>,
        rhs: Box<SdfNode>,
    },
    Subtract {
        lhs: Box<SdfNode>,
        rhs: Box<SdfNode>,
    },
    UnionRound {
        lhs: Box<SdfNode>,
        rhs: Box<SdfNode>,
        r: f32,
    },
    UnionChamfer {
        lhs: Box<SdfNode>,
        rhs: Box<SdfNode>,
        r: f32,
    },
    UnionColumns {
        lhs: Box<SdfNode>,
        rhs: Box<SdfNode>,
        r: f32,
        n: f32,
    },
    UnionStairs {
        lhs: Box<SdfNode>,
        rhs: Box<SdfNode>,
        r: f32,
        n: f32,
    },
    UnionSoft {
        lhs: Box<SdfNode>,
        rhs: Box<SdfNode>,
        r: f32,
    },
    IntersectRound {
        lhs: Box<SdfNode>,
        rhs: Box<SdfNode>,
        r: f32,
    },
    IntersectChamfer {
        lhs: Box<SdfNode>,
        rhs: Box<SdfNode>,
        r: f32,
    },
    IntersectColumns {
        lhs: Box<SdfNode>,
        rhs: Box<SdfNode>,
        r: f32,
        n: f32,
    },
    IntersectStairs {
        lhs: Box<SdfNode>,
        rhs: Box<SdfNode>,
        r: f32,
        n: f32,
    },
    DiffRound {
        lhs: Box<SdfNode>,
        rhs: Box<SdfNode>,
        r: f32,
    },
    DiffChamfer {
        lhs: Box<SdfNode>,
        rhs: Box<SdfNode>,
        r: f32,
    },
    DiffColumns {
        lhs: Box<SdfNode>,
        rhs: Box<SdfNode>,
        r: f32,
        n: f32,
    },
    DiffStairs {
        lhs: Box<SdfNode>,
        rhs: Box<SdfNode>,
        r: f32,
        n: f32,
    },
    Pipe {
        lhs: Box<SdfNode>,
        rhs: Box<SdfNode>,
        r: f32,
    },
    Engrave {
        lhs: Box<SdfNode>,
        rhs: Box<SdfNode>,
        r: f32,
    },
    Groove {
        lhs: Box<SdfNode>,
        rhs: Box<SdfNode>,
        ra: f32,
        rb: f32,
    },
    Tongue {
        lhs: Box<SdfNode>,
        rhs: Box<SdfNode>,
        ra: f32,
        rb: f32,
    },
    Slice {
        base: Box<SdfNode>,
        axis: usize,
        min: f32,
        max: f32,
    },
    Smooth {
        base: Box<SdfNode>,
        k: f32,
    },
}

struct CustomSdfRuntime {
    state: Arc<EvalState>,
    name: String,
    overrides: ObjectValue,
}

struct ModifierFunctionRuntime {
    state: Arc<EvalState>,
    function: FunctionValue,
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

    #[allow(dead_code)]
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

    #[allow(dead_code)]
    fn min(self, rhs: Self) -> Self {
        Self::new(self.x.min(rhs.x), self.y.min(rhs.y), self.z.min(rhs.z))
    }

    #[allow(dead_code)]
    fn max(self, rhs: Self) -> Self {
        Self::new(self.x.max(rhs.x), self.y.max(rhs.y), self.z.max(rhs.z))
    }
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
struct Aabb {
    min: Vec3,
    max: Vec3,
}

#[allow(dead_code)]
impl Aabb {
    fn centroid(self) -> Vec3 {
        self.min.add(self.max).mul(0.5)
    }

    fn extent(self) -> Vec3 {
        self.max.sub(self.min)
    }

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
    materials: Vec<MaterialKindRt>,
    object_transforms: Vec<PrimitiveTransform>,
    dynamic_material_overrides: Vec<ObjectValue>,
    semantic_lights: Vec<SemanticLight>,
}

struct RenderSetup {
    state: EvalState,
    root: SdfNode,
    camera: CameraKind,
    lights: Vec<Box<dyn Light>>,
    path_lights: Vec<PathLight>,
    materials: Vec<MaterialKindRt>,
    object_transforms: Vec<PrimitiveTransform>,
    material_def_names: Vec<String>,
    dynamic_material_overrides: Vec<ObjectValue>,
    environment_name: Option<String>,
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
    dynamic_material_overrides: Vec<ObjectValue>,
    semantic_lights: Vec<SemanticLight>,
}

type MaterialKindRt = Material;

impl CompileContext {
    fn new(default_material: MaterialKindRt, _state: &EvalState) -> Self {
        Self {
            next_object_id: 1,
            default_material,
            materials: vec![default_material],
            object_transforms: vec![PrimitiveTransform::identity()],
            dynamic_material_overrides: Vec::new(),
            semantic_lights: Vec::new(),
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

    fn intern_dynamic_material_override(&mut self, overrides: &ObjectValue) -> Option<u32> {
        if overrides.fields.is_empty() {
            return None;
        }
        if let Some((idx, _)) = self
            .dynamic_material_overrides
            .iter()
            .enumerate()
            .find(|(_, existing)| *existing == overrides)
        {
            Some(idx as u32)
        } else {
            self.dynamic_material_overrides.push(overrides.clone());
            Some((self.dynamic_material_overrides.len() - 1) as u32)
        }
    }
}

#[derive(Clone, Copy)]
struct SemanticLight {
    position: Vec3,
    radius: f32,
    intensity: Spectrum,
    samples: u32,
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

    fn lower_bound(&self, p: Vec3) -> f32;

    fn scene_bounds(&self) -> Aabb;

    fn distance(&self, p: Vec3) -> f32 {
        self.distance_info(p).distance
    }
}

#[derive(Clone)]
struct AccelLeaf {
    bounds: Aabb,
    node: SdfNode,
}

const CUSTOM_LEAF_FAR_LOWER_BOUND_MIN: f32 = 0.05;
const CUSTOM_LEAF_FAR_LOWER_BOUND_MAX: f32 = 0.5;
const CUSTOM_LEAF_FAR_LOWER_BOUND_SCALE: f32 = 0.1;

fn accel_leaf_distance_info(leaf: &AccelLeaf, p: Vec3) -> DistanceInfo {
    if let SdfNode::Custom {
        object_id,
        material_id,
        ..
    } = &leaf.node
    {
        let lb = point_aabb_lower_bound(p, leaf.bounds);
        let far_threshold = (leaf.bounds.extent().length() * CUSTOM_LEAF_FAR_LOWER_BOUND_SCALE)
            .clamp(
                CUSTOM_LEAF_FAR_LOWER_BOUND_MIN,
                CUSTOM_LEAF_FAR_LOWER_BOUND_MAX,
            );
        if lb > far_threshold {
            return DistanceInfo {
                distance: lb,
                object_id: *object_id,
                material_id: *material_id,
            };
        }
    }
    sdf_distance_info(&leaf.node, p)
}

fn collect_accel_leaves(node: &SdfNode, out: &mut Vec<AccelLeaf>) {
    match node {
        SdfNode::Union { lhs, rhs } => {
            collect_accel_leaves(lhs, out);
            collect_accel_leaves(rhs, out);
        }
        _ => out.push(AccelLeaf {
            bounds: sdf_bounds(node),
            node: node.clone(),
        }),
    }
}

#[derive(Clone)]
enum BvhNode {
    Leaf {
        bounds: Aabb,
        leaf_index: usize,
    },
    Inner {
        bounds: Aabb,
        lhs: Box<BvhNode>,
        rhs: Box<BvhNode>,
    },
}

impl BvhNode {
    fn bounds(&self) -> Aabb {
        match self {
            Self::Leaf { bounds, .. } | Self::Inner { bounds, .. } => *bounds,
        }
    }
}

fn build_bvh(leaves: &[AccelLeaf], indices: &[usize]) -> Option<BvhNode> {
    if indices.is_empty() {
        return None;
    }
    if indices.len() == 1 {
        let idx = indices[0];
        return Some(BvhNode::Leaf {
            bounds: leaves[idx].bounds,
            leaf_index: idx,
        });
    }

    let mut bounds = leaves[indices[0]].bounds;
    for &idx in &indices[1..] {
        bounds = bounds.union(leaves[idx].bounds);
    }
    let extent = bounds.extent();
    let axis = if extent.x >= extent.y && extent.x >= extent.z {
        0
    } else if extent.y >= extent.z {
        1
    } else {
        2
    };

    let mut sorted = indices.to_vec();
    sorted.sort_by(|&a, &b| {
        let ca = leaf_centroid_axis(leaves[a].bounds, axis);
        let cb = leaf_centroid_axis(leaves[b].bounds, axis);
        ca.partial_cmp(&cb).unwrap_or(std::cmp::Ordering::Equal)
    });
    let mid = sorted.len() / 2;
    let lhs = build_bvh(leaves, &sorted[..mid])?;
    let rhs = build_bvh(leaves, &sorted[mid..])?;
    Some(BvhNode::Inner {
        bounds,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    })
}

fn leaf_centroid_axis(bounds: Aabb, axis: usize) -> f32 {
    let c = bounds.centroid();
    match axis {
        0 => c.x,
        1 => c.y,
        _ => c.z,
    }
}

fn bvh_distance_info(
    node: &BvhNode,
    leaves: &[AccelLeaf],
    p: Vec3,
    best: &mut f32,
) -> Option<DistanceInfo> {
    let node_lb = point_aabb_lower_bound(p, node.bounds());
    if node_lb > *best {
        return None;
    }
    match node {
        BvhNode::Leaf { leaf_index, .. } => {
            let leaf = &leaves[*leaf_index];
            let info = accel_leaf_distance_info(leaf, p);
            if info.distance < *best {
                *best = info.distance;
            }
            Some(info)
        }
        BvhNode::Inner { lhs, rhs, .. } => {
            let lhs_lb = point_aabb_lower_bound(p, lhs.bounds());
            let rhs_lb = point_aabb_lower_bound(p, rhs.bounds());
            let (first, first_lb, second, second_lb) = if lhs_lb <= rhs_lb {
                (lhs.as_ref(), lhs_lb, rhs.as_ref(), rhs_lb)
            } else {
                (rhs.as_ref(), rhs_lb, lhs.as_ref(), lhs_lb)
            };
            let mut best_info = if first_lb <= *best {
                bvh_distance_info(first, leaves, p, best)
            } else {
                None
            };
            if second_lb <= *best
                && let Some(info) = bvh_distance_info(second, leaves, p, best)
            {
                best_info = match best_info {
                    Some(current) if current.distance <= info.distance => Some(current),
                    _ => Some(info),
                };
            }
            best_info
        }
    }
}

fn bvh_lower_bound(node: &BvhNode, p: Vec3, best: &mut f32) -> Option<f32> {
    let node_lb = point_aabb_lower_bound(p, node.bounds());
    if node_lb > *best {
        return None;
    }
    match node {
        BvhNode::Leaf { bounds, .. } => {
            let lb = point_aabb_lower_bound(p, *bounds);
            if lb < *best {
                *best = lb;
            }
            Some(lb)
        }
        BvhNode::Inner { lhs, rhs, .. } => {
            let lhs_lb = point_aabb_lower_bound(p, lhs.bounds());
            let rhs_lb = point_aabb_lower_bound(p, rhs.bounds());
            let (first, second) = if lhs_lb <= rhs_lb {
                (lhs.as_ref(), rhs.as_ref())
            } else {
                (rhs.as_ref(), lhs.as_ref())
            };
            let mut best_lb = if point_aabb_lower_bound(p, first.bounds()) <= *best {
                bvh_lower_bound(first, p, best)
            } else {
                None
            };
            if point_aabb_lower_bound(p, second.bounds()) <= *best
                && let Some(lb) = bvh_lower_bound(second, p, best)
            {
                best_lb = Some(best_lb.map_or(lb, |current| current.min(lb)));
            }
            best_lb
        }
    }
}

struct BrickGrid {
    bounds: Aabb,
    dims: [usize; 3],
    cells: Vec<Vec<usize>>,
}

impl BrickGrid {
    fn from_leaves(bounds: Aabb, leaves: &[AccelLeaf]) -> Option<Self> {
        if leaves.is_empty() {
            return None;
        }
        let leaf_count = leaves.len().max(1) as f32;
        let dim = leaf_count.cbrt().ceil().max(1.0) as usize;
        let dims = [dim, dim, dim];
        let mut cells = vec![Vec::new(); dims[0] * dims[1] * dims[2]];
        for (leaf_index, leaf) in leaves.iter().enumerate() {
            let min_idx = brick_cell_coords(bounds, dims, leaf.bounds.min);
            let max_idx = brick_cell_coords(bounds, dims, leaf.bounds.max);
            for z in min_idx[2]..=max_idx[2] {
                for y in min_idx[1]..=max_idx[1] {
                    for x in min_idx[0]..=max_idx[0] {
                        let idx = brick_cell_index(dims, [x, y, z]);
                        cells[idx].push(leaf_index);
                    }
                }
            }
        }
        for cell in &mut cells {
            cell.sort_unstable();
            cell.dedup();
        }
        Some(Self {
            bounds,
            dims,
            cells,
        })
    }

    fn distance_info(&self, leaves: &[AccelLeaf], p: Vec3) -> DistanceInfo {
        let origin = brick_cell_coords(self.bounds, self.dims, p);
        let max_shell = self.dims[0].max(self.dims[1]).max(self.dims[2]);
        let mut best = f32::INFINITY;
        let mut best_info = None;
        let mut visited = std::collections::HashSet::new();
        for shell in 0..max_shell {
            let shell_lb = brick_shell_lower_bound(self.bounds, self.dims, origin, shell, p);
            if shell_lb > best {
                break;
            }
            for z in origin[2].saturating_sub(shell)..=(origin[2] + shell).min(self.dims[2] - 1) {
                for y in origin[1].saturating_sub(shell)..=(origin[1] + shell).min(self.dims[1] - 1)
                {
                    for x in
                        origin[0].saturating_sub(shell)..=(origin[0] + shell).min(self.dims[0] - 1)
                    {
                        if shell > 0
                            && x > origin[0].saturating_sub(shell)
                            && x < (origin[0] + shell).min(self.dims[0] - 1)
                            && y > origin[1].saturating_sub(shell)
                            && y < (origin[1] + shell).min(self.dims[1] - 1)
                            && z > origin[2].saturating_sub(shell)
                            && z < (origin[2] + shell).min(self.dims[2] - 1)
                        {
                            continue;
                        }
                        let idx = brick_cell_index(self.dims, [x, y, z]);
                        for &leaf_index in &self.cells[idx] {
                            if !visited.insert(leaf_index) {
                                continue;
                            }
                            let leaf = &leaves[leaf_index];
                            let lb = point_aabb_lower_bound(p, leaf.bounds);
                            if lb > best {
                                continue;
                            }
                            let info = accel_leaf_distance_info(leaf, p);
                            if info.distance < best {
                                best = info.distance;
                                best_info = Some(info);
                            }
                        }
                    }
                }
            }
        }
        best_info.unwrap_or_else(|| sdf_distance_info(&leaves[0].node, p))
    }

    fn lower_bound(&self, leaves: &[AccelLeaf], p: Vec3) -> f32 {
        let origin = brick_cell_coords(self.bounds, self.dims, p);
        let max_shell = self.dims[0].max(self.dims[1]).max(self.dims[2]);
        let mut best = f32::INFINITY;
        let mut visited = std::collections::HashSet::new();
        for shell in 0..max_shell {
            let shell_lb = brick_shell_lower_bound(self.bounds, self.dims, origin, shell, p);
            if shell_lb > best {
                break;
            }
            for z in origin[2].saturating_sub(shell)..=(origin[2] + shell).min(self.dims[2] - 1) {
                for y in origin[1].saturating_sub(shell)..=(origin[1] + shell).min(self.dims[1] - 1)
                {
                    for x in
                        origin[0].saturating_sub(shell)..=(origin[0] + shell).min(self.dims[0] - 1)
                    {
                        if shell > 0
                            && x > origin[0].saturating_sub(shell)
                            && x < (origin[0] + shell).min(self.dims[0] - 1)
                            && y > origin[1].saturating_sub(shell)
                            && y < (origin[1] + shell).min(self.dims[1] - 1)
                            && z > origin[2].saturating_sub(shell)
                            && z < (origin[2] + shell).min(self.dims[2] - 1)
                        {
                            continue;
                        }
                        let idx = brick_cell_index(self.dims, [x, y, z]);
                        for &leaf_index in &self.cells[idx] {
                            if !visited.insert(leaf_index) {
                                continue;
                            }
                            let lb = point_aabb_lower_bound(p, leaves[leaf_index].bounds);
                            if lb < best {
                                best = lb;
                            }
                        }
                    }
                }
            }
        }
        best
    }
}

fn brick_cell_index(dims: [usize; 3], coords: [usize; 3]) -> usize {
    coords[0] + dims[0] * (coords[1] + dims[1] * coords[2])
}

fn brick_cell_coords(bounds: Aabb, dims: [usize; 3], p: Vec3) -> [usize; 3] {
    let extent = bounds.extent();
    let coord = |value: f32, min: f32, extent: f32, dim: usize| {
        if extent <= 1.0e-6 {
            return 0;
        }
        (((value - min) / extent).clamp(0.0, 0.999_999) * dim as f32) as usize
    };
    [
        coord(p.x, bounds.min.x, extent.x, dims[0]),
        coord(p.y, bounds.min.y, extent.y, dims[1]),
        coord(p.z, bounds.min.z, extent.z, dims[2]),
    ]
}

fn brick_cell_aabb(bounds: Aabb, dims: [usize; 3], coords: [usize; 3]) -> Aabb {
    let extent = bounds.extent();
    let size = Vec3::new(
        extent.x / dims[0].max(1) as f32,
        extent.y / dims[1].max(1) as f32,
        extent.z / dims[2].max(1) as f32,
    );
    let min = Vec3::new(
        bounds.min.x + size.x * coords[0] as f32,
        bounds.min.y + size.y * coords[1] as f32,
        bounds.min.z + size.z * coords[2] as f32,
    );
    Aabb {
        min,
        max: min.add(size),
    }
}

fn brick_shell_lower_bound(
    bounds: Aabb,
    dims: [usize; 3],
    origin: [usize; 3],
    shell: usize,
    p: Vec3,
) -> f32 {
    if shell == 0 {
        return 0.0;
    }
    let min_coords = [
        origin[0].saturating_sub(shell),
        origin[1].saturating_sub(shell),
        origin[2].saturating_sub(shell),
    ];
    let max_coords = [
        (origin[0] + shell).min(dims[0] - 1),
        (origin[1] + shell).min(dims[1] - 1),
        (origin[2] + shell).min(dims[2] - 1),
    ];
    let min_aabb = brick_cell_aabb(bounds, dims, min_coords);
    let max_aabb = brick_cell_aabb(bounds, dims, max_coords);
    point_aabb_lower_bound(
        p,
        Aabb {
            min: min_aabb.min,
            max: max_aabb.max,
        },
    )
}

struct NaiveAccel {
    scene: CompiledScene,
    bounds: Aabb,
}

impl Accelerator for NaiveAccel {
    fn from_scene(scene: CompiledScene) -> Self {
        let bounds = sdf_bounds(&scene.root);
        Self { scene, bounds }
    }

    fn distance_info(&self, p: Vec3) -> DistanceInfo {
        sdf_distance_info(&self.scene.root, p)
    }

    fn lower_bound(&self, p: Vec3) -> f32 {
        sdf_lower_bound(&self.scene.root, p)
    }

    fn scene_bounds(&self) -> Aabb {
        self.bounds
    }
}

struct BvhAccel {
    scene: CompiledScene,
    bounds: Aabb,
    leaves: Vec<AccelLeaf>,
    root: Option<BvhNode>,
}

impl Accelerator for BvhAccel {
    fn from_scene(scene: CompiledScene) -> Self {
        let bounds = sdf_bounds(&scene.root);
        let mut leaves = Vec::new();
        collect_accel_leaves(&scene.root, &mut leaves);
        let indices = (0..leaves.len()).collect::<Vec<_>>();
        let root = build_bvh(&leaves, &indices);
        Self {
            scene,
            bounds,
            leaves,
            root,
        }
    }

    fn distance_info(&self, p: Vec3) -> DistanceInfo {
        let mut best = f32::INFINITY;
        self.root
            .as_ref()
            .and_then(|root| bvh_distance_info(root, &self.leaves, p, &mut best))
            .unwrap_or_else(|| sdf_distance_info(&self.scene.root, p))
    }

    fn lower_bound(&self, p: Vec3) -> f32 {
        let mut best = f32::INFINITY;
        self.root
            .as_ref()
            .and_then(|root| bvh_lower_bound(root, p, &mut best))
            .unwrap_or_else(|| sdf_lower_bound(&self.scene.root, p))
    }

    fn scene_bounds(&self) -> Aabb {
        self.bounds
    }
}

struct BricksAccel {
    scene: CompiledScene,
    bounds: Aabb,
    leaves: Vec<AccelLeaf>,
    grid: Option<BrickGrid>,
}

impl Accelerator for BricksAccel {
    fn from_scene(scene: CompiledScene) -> Self {
        let bounds = sdf_bounds(&scene.root);
        let mut leaves = Vec::new();
        collect_accel_leaves(&scene.root, &mut leaves);
        let grid = BrickGrid::from_leaves(bounds, &leaves);
        Self {
            scene,
            bounds,
            leaves,
            grid,
        }
    }

    fn distance_info(&self, p: Vec3) -> DistanceInfo {
        self.grid
            .as_ref()
            .map(|grid| grid.distance_info(&self.leaves, p))
            .unwrap_or_else(|| sdf_distance_info(&self.scene.root, p))
    }

    fn lower_bound(&self, p: Vec3) -> f32 {
        self.grid
            .as_ref()
            .map(|grid| grid.lower_bound(&self.leaves, p))
            .unwrap_or_else(|| sdf_lower_bound(&self.scene.root, p))
    }

    fn scene_bounds(&self) -> Aabb {
        self.bounds
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

#[allow(dead_code)]
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
        aa_samples: 1,
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
    aa_samples: u32,
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
            aa_samples,
            &mut on_tile,
        )?,
        AccelMode::Bvh => render_preview_with_accel_progressive::<BvhAccel>(
            scene,
            setup,
            options,
            tile_size,
            aa_samples,
            &mut on_tile,
        )?,
        AccelMode::Bricks => render_preview_with_accel_progressive::<BricksAccel>(
            scene,
            setup,
            options,
            tile_size,
            aa_samples,
            &mut on_tile,
        )?,
    };
    Ok(image)
}

#[allow(dead_code)]
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
    out.step_scale = read_number_field(obj, &["step_scale"]);
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
        Value::String(_) | Value::Array(_) | Value::Function(_) => None,
    }
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
    Ok(CompiledScene {
        root,
        center,
        materials: ctx.materials,
        object_transforms: ctx.object_transforms,
        dynamic_material_overrides: ctx.dynamic_material_overrides,
        semantic_lights: ctx.semantic_lights,
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

    let node = match type_name {
        "Sphere" => {
            let transform = read_transform(object);
            let radius = read_number_field(object, &["radius", "r"]).unwrap_or(1.0_f32);
            let shell = read_number_field(object, &["shell"])
                .unwrap_or(0.0)
                .max(0.0)
                .min(radius);
            let object_id = ctx.alloc_object_id();
            ctx.register_object_transform(object_id, transform);
            let material_id = primitive_material_id(state, object, ctx);
            Ok(SdfNode::Sphere {
                transform,
                radius,
                shell,
                object_id,
                material_id,
            })
        }
        "Box" => {
            let transform = read_transform(object);
            let size = read_vec3_field(object, "size").unwrap_or(Vec3::new(1.0, 1.0, 1.0));
            let half_size = size.mul(0.5);
            let round = read_number_field(object, &["round", "rounding"])
                .unwrap_or(0.0)
                .max(0.0)
                .min(half_size.x.min(half_size.y).min(half_size.z));
            let shell = read_number_field(object, &["shell"])
                .unwrap_or(0.0)
                .max(0.0)
                .min(half_size.x.min(half_size.y).min(half_size.z));
            let object_id = ctx.alloc_object_id();
            ctx.register_object_transform(object_id, transform);
            let material_id = primitive_material_id(state, object, ctx);
            Ok(SdfNode::Box {
                transform,
                half_size,
                round,
                shell,
                object_id,
                material_id,
            })
        }
        "Cylinder" => {
            let transform = read_transform(object);
            let radius = read_number_field(object, &["radius", "r"]).unwrap_or(1.0);
            let height = read_number_field(object, &["height", "h"]).unwrap_or(1.0);
            let half_height = height * 0.5;
            let round = read_number_field(object, &["round", "rounding"])
                .unwrap_or(0.0)
                .max(0.0)
                .min(radius.min(half_height));
            let shell = read_number_field(object, &["shell"])
                .unwrap_or(0.0)
                .max(0.0)
                .min(radius.min(half_height));
            let object_id = ctx.alloc_object_id();
            ctx.register_object_transform(object_id, transform);
            let material_id = primitive_material_id(state, object, ctx);
            Ok(SdfNode::Cylinder {
                transform,
                radius,
                half_height,
                round,
                shell,
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
        "ExtrudePolygon" => {
            let transform = read_transform(object);
            let sides = read_number_field(object, &["sides", "n"])
                .map(|v| v.round() as u32)
                .unwrap_or(6)
                .max(3);
            let radius = read_number_field(object, &["radius", "r"]).unwrap_or(1.0);
            let height = read_number_field(object, &["height", "h"]).unwrap_or(1.0);
            let half_height = height * 0.5;
            let round = read_number_field(object, &["round", "rounding"])
                .unwrap_or(0.0)
                .max(0.0)
                .min(radius.min(half_height));
            let shell = read_number_field(object, &["shell"])
                .unwrap_or(0.0)
                .max(0.0)
                .min(radius.min(half_height));
            let object_id = ctx.alloc_object_id();
            ctx.register_object_transform(object_id, transform);
            let material_id = primitive_material_id(state, object, ctx);
            Ok(SdfNode::ExtrudePolygon {
                transform,
                sides,
                radius,
                half_height,
                round,
                shell,
                object_id,
                material_id,
            })
        }
        "Room" => compile_room(state, object, ctx),
        custom if state.sdf_defs.contains_key(custom) => {
            if let Some(lowered) = compile_lowered_library_object(state, custom, object, ctx) {
                lowered
            } else {
                let transform = read_transform(object);
                let object_id = ctx.alloc_object_id();
                ctx.register_object_transform(object_id, transform);
                let material_id = primitive_material_id(state, object, ctx);
                Ok(SdfNode::Custom {
                    transform,
                    runtime: Arc::new(CustomSdfRuntime {
                        state: Arc::clone(state),
                        name: custom.to_string(),
                        overrides: object.clone(),
                    }),
                    bounds_half_extents: eval_custom_sdf_bounds_half_extents(state, custom, object),
                    object_id,
                    material_id,
                })
            }
        }
        "add" => {
            let lhs = compile_sdf(state, required_field(object, "lhs")?, ctx)?;
            let rhs = compile_sdf(state, required_field(object, "rhs")?, ctx)?;
            Ok(SdfNode::Union {
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            })
        }
        "intersect" => {
            let lhs = compile_sdf(state, required_field(object, "lhs")?, ctx)?;
            let rhs = compile_sdf(state, required_field(object, "rhs")?, ctx)?;
            Ok(SdfNode::Intersect {
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
        "union_round" | "union_chamfer" | "union_soft" | "intersect_round"
        | "intersect_chamfer" | "diff_round" | "diff_chamfer" | "pipe" | "engrave" => {
            let lhs = compile_sdf(state, required_field(object, "lhs")?, ctx)?;
            let rhs = compile_sdf(state, required_field(object, "rhs")?, ctx)?;
            let r = match required_field(object, "r")? {
                Value::Number(v) => *v as f32,
                _ => 0.0,
            };
            let lhs = Box::new(lhs);
            let rhs = Box::new(rhs);
            Ok(match type_name {
                "union_round" => SdfNode::UnionRound { lhs, rhs, r },
                "union_chamfer" => SdfNode::UnionChamfer { lhs, rhs, r },
                "union_soft" => SdfNode::UnionSoft { lhs, rhs, r },
                "intersect_round" => SdfNode::IntersectRound { lhs, rhs, r },
                "intersect_chamfer" => SdfNode::IntersectChamfer { lhs, rhs, r },
                "diff_round" => SdfNode::DiffRound { lhs, rhs, r },
                "diff_chamfer" => SdfNode::DiffChamfer { lhs, rhs, r },
                "pipe" => SdfNode::Pipe { lhs, rhs, r },
                "engrave" => SdfNode::Engrave { lhs, rhs, r },
                _ => unreachable!(),
            })
        }
        "union_columns" | "union_stairs" | "intersect_columns" | "intersect_stairs"
        | "diff_columns" | "diff_stairs" => {
            let lhs = compile_sdf(state, required_field(object, "lhs")?, ctx)?;
            let rhs = compile_sdf(state, required_field(object, "rhs")?, ctx)?;
            let r = match required_field(object, "r")? {
                Value::Number(v) => *v as f32,
                _ => 0.0,
            };
            let n = match required_field(object, "n")? {
                Value::Number(v) => *v as f32,
                _ => 4.0,
            };
            let lhs = Box::new(lhs);
            let rhs = Box::new(rhs);
            Ok(match type_name {
                "union_columns" => SdfNode::UnionColumns { lhs, rhs, r, n },
                "union_stairs" => SdfNode::UnionStairs { lhs, rhs, r, n },
                "intersect_columns" => SdfNode::IntersectColumns { lhs, rhs, r, n },
                "intersect_stairs" => SdfNode::IntersectStairs { lhs, rhs, r, n },
                "diff_columns" => SdfNode::DiffColumns { lhs, rhs, r, n },
                "diff_stairs" => SdfNode::DiffStairs { lhs, rhs, r, n },
                _ => unreachable!(),
            })
        }
        "groove" | "tongue" => {
            let lhs = compile_sdf(state, required_field(object, "lhs")?, ctx)?;
            let rhs = compile_sdf(state, required_field(object, "rhs")?, ctx)?;
            let ra = match required_field(object, "ra")? {
                Value::Number(v) => *v as f32,
                _ => 0.0,
            };
            let rb = match required_field(object, "rb")? {
                Value::Number(v) => *v as f32,
                _ => 0.0,
            };
            let lhs = Box::new(lhs);
            let rhs = Box::new(rhs);
            Ok(match type_name {
                "groove" => SdfNode::Groove { lhs, rhs, ra, rb },
                "tongue" => SdfNode::Tongue { lhs, rhs, ra, rb },
                _ => unreachable!(),
            })
        }
        "mirror_x" | "mirror_y" | "mirror_z" => {
            let base_value = required_field(object, "base")?;
            let lhs = compile_sdf(state, base_value, ctx)?;
            let axis = match type_name {
                "mirror_x" => 0,
                "mirror_y" => 1,
                _ => 2,
            };
            let rhs = compile_sdf_mirrored(state, base_value, ctx, axis)?;
            Ok(SdfNode::Union {
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            })
        }
        "repeat_x" | "repeat_y" | "repeat_z" => {
            let base_value = required_field(object, "base")?;
            let spacing = match required_field(object, "spacing")? {
                Value::Number(v) => *v as f32,
                _ => 0.0,
            };
            let count = match required_field(object, "count")? {
                Value::Number(v) => v.round().max(1.0) as usize,
                _ => 1,
            };
            let axis = match type_name {
                "repeat_x" => 0,
                "repeat_y" => 1,
                _ => 2,
            };
            let start = -0.5 * (count.saturating_sub(1) as f32) * spacing;
            let mut root = compile_sdf_offset(state, base_value, ctx, axis, start)?;
            for i in 1..count {
                let part =
                    compile_sdf_offset(state, base_value, ctx, axis, start + i as f32 * spacing)?;
                root = SdfNode::Union {
                    lhs: Box::new(root),
                    rhs: Box::new(part),
                };
            }
            Ok(root)
        }
        "slice_x" | "slice_y" | "slice_z" => {
            let base = compile_sdf(state, required_field(object, "base")?, ctx)?;
            let min = match required_field(object, "min")? {
                Value::Number(v) => *v as f32,
                _ => f32::NEG_INFINITY,
            };
            let max = match required_field(object, "max")? {
                Value::Number(v) => *v as f32,
                _ => f32::INFINITY,
            };
            let axis = match type_name {
                "slice_x" => 0,
                "slice_y" => 1,
                _ => 2,
            };
            Ok(SdfNode::Slice {
                base: Box::new(base),
                axis,
                min,
                max,
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
        other => Err(RenderError::UnsupportedObjectType(other.to_string())),
    }?;

    apply_object_modifier_hooks(state, object, node)
}

fn required_field<'a>(obj: &'a ObjectValue, name: &str) -> Result<&'a Value, RenderError> {
    obj.fields.get(name).ok_or_else(|| {
        RenderError::UnsupportedObjectType(obj.type_name.clone().unwrap_or_default())
    })
}

fn apply_object_modifier_hooks(
    state: &Arc<EvalState>,
    object: &ObjectValue,
    mut node: SdfNode,
) -> Result<SdfNode, RenderError> {
    if object.fields.contains_key("domain") || object.fields.contains_key("distance_post") {
        let (base, transform, bounds) = split_modifier_base(node);
        node = base;
        if let Some(Value::Function(function)) = object.fields.get("domain") {
            node = SdfNode::DomainModifier {
                base: Box::new(node),
                runtime: Arc::new(ModifierFunctionRuntime {
                    state: Arc::clone(state),
                    function: function.clone(),
                }),
                transform,
                bounds,
            };
        }
        if let Some(Value::Function(function)) = object.fields.get("distance_post") {
            node = SdfNode::DistancePostModifier {
                base: Box::new(node),
                runtime: Arc::new(ModifierFunctionRuntime {
                    state: Arc::clone(state),
                    function: function.clone(),
                }),
                transform,
                bounds,
            };
        }
        return Ok(node);
    }

    if let Some(Value::Function(function)) = object.fields.get("domain") {
        let bounds = sdf_bounds(&node);
        node = SdfNode::DomainModifier {
            base: Box::new(node),
            runtime: Arc::new(ModifierFunctionRuntime {
                state: Arc::clone(state),
                function: function.clone(),
            }),
            transform: PrimitiveTransform::identity(),
            bounds,
        };
    }
    if let Some(Value::Function(function)) = object.fields.get("distance_post") {
        let bounds = sdf_bounds(&node);
        node = SdfNode::DistancePostModifier {
            base: Box::new(node),
            runtime: Arc::new(ModifierFunctionRuntime {
                state: Arc::clone(state),
                function: function.clone(),
            }),
            transform: PrimitiveTransform::identity(),
            bounds,
        };
    }
    Ok(node)
}

fn split_modifier_base(node: SdfNode) -> (SdfNode, PrimitiveTransform, Aabb) {
    let bounds = sdf_bounds(&node);
    match node {
        SdfNode::Sphere {
            radius,
            shell,
            object_id,
            material_id,
            transform,
        } => (
            SdfNode::Sphere {
                transform: PrimitiveTransform::identity(),
                radius,
                shell,
                object_id,
                material_id,
            },
            transform,
            bounds,
        ),
        SdfNode::Box {
            half_size,
            round,
            shell,
            object_id,
            material_id,
            transform,
        } => (
            SdfNode::Box {
                transform: PrimitiveTransform::identity(),
                half_size,
                round,
                shell,
                object_id,
                material_id,
            },
            transform,
            bounds,
        ),
        SdfNode::Cylinder {
            radius,
            half_height,
            round,
            shell,
            object_id,
            material_id,
            transform,
        } => (
            SdfNode::Cylinder {
                transform: PrimitiveTransform::identity(),
                radius,
                half_height,
                round,
                shell,
                object_id,
                material_id,
            },
            transform,
            bounds,
        ),
        SdfNode::Torus {
            major_radius,
            minor_radius,
            object_id,
            material_id,
            transform,
        } => (
            SdfNode::Torus {
                transform: PrimitiveTransform::identity(),
                major_radius,
                minor_radius,
                object_id,
                material_id,
            },
            transform,
            bounds,
        ),
        SdfNode::ExtrudePolygon {
            sides,
            radius,
            half_height,
            round,
            shell,
            object_id,
            material_id,
            transform,
        } => (
            SdfNode::ExtrudePolygon {
                transform: PrimitiveTransform::identity(),
                sides,
                radius,
                half_height,
                round,
                shell,
                object_id,
                material_id,
            },
            transform,
            bounds,
        ),
        SdfNode::Custom {
            runtime,
            bounds_half_extents,
            object_id,
            material_id,
            transform,
        } => (
            SdfNode::Custom {
                transform: PrimitiveTransform::identity(),
                runtime,
                bounds_half_extents,
                object_id,
                material_id,
            },
            transform,
            bounds,
        ),
        other => (other, PrimitiveTransform::identity(), bounds),
    }
}

fn eval_modifier_domain(runtime: &ModifierFunctionRuntime, p: Vec3) -> Vec3 {
    match eval_function_value(&runtime.state, &runtime.function, &[vec3_value_value(p)]) {
        Ok(Value::Object(obj)) => {
            let x = read_number_field(&obj, &["x"]).unwrap_or(p.x);
            let y = read_number_field(&obj, &["y"]).unwrap_or(p.y);
            let z = read_number_field(&obj, &["z"]).unwrap_or(p.z);
            Vec3::new(x, y, z)
        }
        _ => p,
    }
}

fn eval_modifier_distance_post(runtime: &ModifierFunctionRuntime, d: f32, p: Vec3) -> f32 {
    match eval_function_value(
        &runtime.state,
        &runtime.function,
        &[Value::Number(d as f64), vec3_value_value(p)],
    ) {
        Ok(Value::Number(v)) => v as f32,
        _ => d,
    }
}

fn compile_sdf_offset(
    state: &Arc<EvalState>,
    value: &Value,
    ctx: &mut CompileContext,
    axis: usize,
    delta: f32,
) -> Result<SdfNode, RenderError> {
    let base = compile_sdf(state, value, ctx)?;
    Ok(remap_sdf_node(base, ctx, |transform| {
        let mut center = transform.center;
        match axis {
            0 => center.x += delta,
            1 => center.y += delta,
            _ => center.z += delta,
        }
        PrimitiveTransform {
            center,
            rot_deg: transform.rot_deg,
        }
    }))
}

fn compile_sdf_mirrored(
    state: &Arc<EvalState>,
    value: &Value,
    ctx: &mut CompileContext,
    axis: usize,
) -> Result<SdfNode, RenderError> {
    let base = compile_sdf(state, value, ctx)?;
    Ok(remap_sdf_node(base, ctx, |transform| {
        let mut center = transform.center;
        let mut rot = transform.rot_deg;
        match axis {
            0 => {
                center.x = -center.x;
                rot.y = -rot.y;
                rot.z = -rot.z;
            }
            1 => {
                center.y = -center.y;
                rot.x = -rot.x;
                rot.z = -rot.z;
            }
            _ => {
                center.z = -center.z;
                rot.x = -rot.x;
                rot.y = -rot.y;
            }
        }
        PrimitiveTransform {
            center,
            rot_deg: rot,
        }
    }))
}

fn remap_sdf_node(
    node: SdfNode,
    ctx: &mut CompileContext,
    map_transform: impl Copy + Fn(PrimitiveTransform) -> PrimitiveTransform,
) -> SdfNode {
    match node {
        SdfNode::Sphere {
            transform,
            radius,
            shell,
            material_id,
            ..
        } => {
            let transform = map_transform(transform);
            let object_id = ctx.alloc_object_id();
            ctx.register_object_transform(object_id, transform);
            SdfNode::Sphere {
                transform,
                radius,
                shell,
                object_id,
                material_id,
            }
        }
        SdfNode::Box {
            transform,
            half_size,
            round,
            shell,
            material_id,
            ..
        } => {
            let transform = map_transform(transform);
            let object_id = ctx.alloc_object_id();
            ctx.register_object_transform(object_id, transform);
            SdfNode::Box {
                transform,
                half_size,
                round,
                shell,
                object_id,
                material_id,
            }
        }
        SdfNode::Cylinder {
            transform,
            radius,
            half_height,
            round,
            shell,
            material_id,
            ..
        } => {
            let transform = map_transform(transform);
            let object_id = ctx.alloc_object_id();
            ctx.register_object_transform(object_id, transform);
            SdfNode::Cylinder {
                transform,
                radius,
                half_height,
                round,
                shell,
                object_id,
                material_id,
            }
        }
        SdfNode::Torus {
            transform,
            major_radius,
            minor_radius,
            material_id,
            ..
        } => {
            let transform = map_transform(transform);
            let object_id = ctx.alloc_object_id();
            ctx.register_object_transform(object_id, transform);
            SdfNode::Torus {
                transform,
                major_radius,
                minor_radius,
                object_id,
                material_id,
            }
        }
        SdfNode::ExtrudePolygon {
            transform,
            sides,
            radius,
            half_height,
            round,
            shell,
            material_id,
            ..
        } => {
            let transform = map_transform(transform);
            let object_id = ctx.alloc_object_id();
            ctx.register_object_transform(object_id, transform);
            SdfNode::ExtrudePolygon {
                transform,
                sides,
                radius,
                half_height,
                round,
                shell,
                object_id,
                material_id,
            }
        }
        SdfNode::Custom {
            transform,
            runtime,
            bounds_half_extents,
            material_id,
            ..
        } => {
            let transform = map_transform(transform);
            let object_id = ctx.alloc_object_id();
            ctx.register_object_transform(object_id, transform);
            SdfNode::Custom {
                transform,
                runtime,
                bounds_half_extents,
                object_id,
                material_id,
            }
        }
        SdfNode::DomainModifier {
            base,
            runtime,
            transform,
            ..
        } => {
            let transform = map_transform(transform);
            let bounds = transformed_modifier_bounds(base.as_ref(), transform);
            SdfNode::DomainModifier {
                base,
                runtime,
                transform,
                bounds,
            }
        }
        SdfNode::DistancePostModifier {
            base,
            runtime,
            transform,
            ..
        } => {
            let transform = map_transform(transform);
            let bounds = transformed_modifier_bounds(base.as_ref(), transform);
            SdfNode::DistancePostModifier {
                base,
                runtime,
                transform,
                bounds,
            }
        }
        SdfNode::Union { lhs, rhs } => SdfNode::Union {
            lhs: Box::new(remap_sdf_node(*lhs, ctx, map_transform)),
            rhs: Box::new(remap_sdf_node(*rhs, ctx, map_transform)),
        },
        SdfNode::Intersect { lhs, rhs } => SdfNode::Intersect {
            lhs: Box::new(remap_sdf_node(*lhs, ctx, map_transform)),
            rhs: Box::new(remap_sdf_node(*rhs, ctx, map_transform)),
        },
        SdfNode::Subtract { lhs, rhs } => SdfNode::Subtract {
            lhs: Box::new(remap_sdf_node(*lhs, ctx, map_transform)),
            rhs: Box::new(remap_sdf_node(*rhs, ctx, map_transform)),
        },
        SdfNode::UnionRound { lhs, rhs, r } => SdfNode::UnionRound {
            lhs: Box::new(remap_sdf_node(*lhs, ctx, map_transform)),
            rhs: Box::new(remap_sdf_node(*rhs, ctx, map_transform)),
            r,
        },
        SdfNode::UnionChamfer { lhs, rhs, r } => SdfNode::UnionChamfer {
            lhs: Box::new(remap_sdf_node(*lhs, ctx, map_transform)),
            rhs: Box::new(remap_sdf_node(*rhs, ctx, map_transform)),
            r,
        },
        SdfNode::UnionColumns { lhs, rhs, r, n } => SdfNode::UnionColumns {
            lhs: Box::new(remap_sdf_node(*lhs, ctx, map_transform)),
            rhs: Box::new(remap_sdf_node(*rhs, ctx, map_transform)),
            r,
            n,
        },
        SdfNode::UnionStairs { lhs, rhs, r, n } => SdfNode::UnionStairs {
            lhs: Box::new(remap_sdf_node(*lhs, ctx, map_transform)),
            rhs: Box::new(remap_sdf_node(*rhs, ctx, map_transform)),
            r,
            n,
        },
        SdfNode::UnionSoft { lhs, rhs, r } => SdfNode::UnionSoft {
            lhs: Box::new(remap_sdf_node(*lhs, ctx, map_transform)),
            rhs: Box::new(remap_sdf_node(*rhs, ctx, map_transform)),
            r,
        },
        SdfNode::IntersectRound { lhs, rhs, r } => SdfNode::IntersectRound {
            lhs: Box::new(remap_sdf_node(*lhs, ctx, map_transform)),
            rhs: Box::new(remap_sdf_node(*rhs, ctx, map_transform)),
            r,
        },
        SdfNode::IntersectChamfer { lhs, rhs, r } => SdfNode::IntersectChamfer {
            lhs: Box::new(remap_sdf_node(*lhs, ctx, map_transform)),
            rhs: Box::new(remap_sdf_node(*rhs, ctx, map_transform)),
            r,
        },
        SdfNode::IntersectColumns { lhs, rhs, r, n } => SdfNode::IntersectColumns {
            lhs: Box::new(remap_sdf_node(*lhs, ctx, map_transform)),
            rhs: Box::new(remap_sdf_node(*rhs, ctx, map_transform)),
            r,
            n,
        },
        SdfNode::IntersectStairs { lhs, rhs, r, n } => SdfNode::IntersectStairs {
            lhs: Box::new(remap_sdf_node(*lhs, ctx, map_transform)),
            rhs: Box::new(remap_sdf_node(*rhs, ctx, map_transform)),
            r,
            n,
        },
        SdfNode::DiffRound { lhs, rhs, r } => SdfNode::DiffRound {
            lhs: Box::new(remap_sdf_node(*lhs, ctx, map_transform)),
            rhs: Box::new(remap_sdf_node(*rhs, ctx, map_transform)),
            r,
        },
        SdfNode::DiffChamfer { lhs, rhs, r } => SdfNode::DiffChamfer {
            lhs: Box::new(remap_sdf_node(*lhs, ctx, map_transform)),
            rhs: Box::new(remap_sdf_node(*rhs, ctx, map_transform)),
            r,
        },
        SdfNode::DiffColumns { lhs, rhs, r, n } => SdfNode::DiffColumns {
            lhs: Box::new(remap_sdf_node(*lhs, ctx, map_transform)),
            rhs: Box::new(remap_sdf_node(*rhs, ctx, map_transform)),
            r,
            n,
        },
        SdfNode::DiffStairs { lhs, rhs, r, n } => SdfNode::DiffStairs {
            lhs: Box::new(remap_sdf_node(*lhs, ctx, map_transform)),
            rhs: Box::new(remap_sdf_node(*rhs, ctx, map_transform)),
            r,
            n,
        },
        SdfNode::Pipe { lhs, rhs, r } => SdfNode::Pipe {
            lhs: Box::new(remap_sdf_node(*lhs, ctx, map_transform)),
            rhs: Box::new(remap_sdf_node(*rhs, ctx, map_transform)),
            r,
        },
        SdfNode::Engrave { lhs, rhs, r } => SdfNode::Engrave {
            lhs: Box::new(remap_sdf_node(*lhs, ctx, map_transform)),
            rhs: Box::new(remap_sdf_node(*rhs, ctx, map_transform)),
            r,
        },
        SdfNode::Groove { lhs, rhs, ra, rb } => SdfNode::Groove {
            lhs: Box::new(remap_sdf_node(*lhs, ctx, map_transform)),
            rhs: Box::new(remap_sdf_node(*rhs, ctx, map_transform)),
            ra,
            rb,
        },
        SdfNode::Tongue { lhs, rhs, ra, rb } => SdfNode::Tongue {
            lhs: Box::new(remap_sdf_node(*lhs, ctx, map_transform)),
            rhs: Box::new(remap_sdf_node(*rhs, ctx, map_transform)),
            ra,
            rb,
        },
        SdfNode::Slice {
            base,
            axis,
            min,
            max,
        } => SdfNode::Slice {
            base: Box::new(remap_sdf_node(*base, ctx, map_transform)),
            axis,
            min,
            max,
        },
        SdfNode::Smooth { base, k } => SdfNode::Smooth {
            base: Box::new(remap_sdf_node(*base, ctx, map_transform)),
            k,
        },
    }
}

fn primitive_material_id(
    state: &Arc<EvalState>,
    obj: &ObjectValue,
    ctx: &mut CompileContext,
) -> u32 {
    if let Some(Value::Object(mat_obj)) = obj.fields.get("material") {
        let material = material_from_object(state, mat_obj, Some(ctx));
        return ctx.intern_material(material);
    }
    ctx.intern_material(ctx.default_material)
}

fn room_material_id(
    state: &Arc<EvalState>,
    obj: &ObjectValue,
    specific_field: &str,
    fallback_field: &str,
    ctx: &mut CompileContext,
) -> u32 {
    if let Some(Value::Object(mat_obj)) = obj.fields.get(specific_field) {
        let material = material_from_object(state, mat_obj, Some(ctx));
        return ctx.intern_material(material);
    }
    if let Some(Value::Object(mat_obj)) = obj.fields.get(fallback_field) {
        let material = material_from_object(state, mat_obj, Some(ctx));
        return ctx.intern_material(material);
    }
    ctx.intern_material(ctx.default_material)
}

fn transformed_modifier_bounds(base: &SdfNode, transform: PrimitiveTransform) -> Aabb {
    let local_bounds = sdf_bounds(base);
    let local_center = local_bounds.centroid();
    let local_radius = local_bounds.extent().mul(0.5).length();
    let center = transform
        .center
        .add(transform_offset(transform, local_center));
    let r = Vec3::new(local_radius, local_radius, local_radius);
    Aabb {
        min: center.sub(r),
        max: center.add(r),
    }
    .expand(0.1)
}

fn object_material_id(
    state: &Arc<EvalState>,
    obj: &ObjectValue,
    specific_field: &str,
    ctx: &mut CompileContext,
) -> u32 {
    if let Some(Value::Object(mat_obj)) = obj.fields.get(specific_field) {
        let material = material_from_object(state, mat_obj, Some(ctx));
        return ctx.intern_material(material);
    }
    primitive_material_id(state, obj, ctx)
}

fn object_material_id_with_fallback(
    state: &Arc<EvalState>,
    obj: &ObjectValue,
    specific_field: &str,
    fallback_field: &str,
    ctx: &mut CompileContext,
) -> u32 {
    if let Some(Value::Object(mat_obj)) = obj.fields.get(specific_field) {
        let material = material_from_object(state, mat_obj, Some(ctx));
        return ctx.intern_material(material);
    }
    if let Some(Value::Object(mat_obj)) = obj.fields.get(fallback_field) {
        let material = material_from_object(state, mat_obj, Some(ctx));
        return ctx.intern_material(material);
    }
    primitive_material_id(state, obj, ctx)
}

fn room_flag(obj: &ObjectValue, name: &str, default: bool) -> bool {
    obj.fields
        .get(name)
        .and_then(|value| match value {
            Value::Number(v) => Some(*v >= 0.5),
            _ => None,
        })
        .unwrap_or(default)
}

fn compile_lowered_library_object(
    state: &Arc<EvalState>,
    name: &str,
    object: &ObjectValue,
    ctx: &mut CompileContext,
) -> Option<Result<SdfNode, RenderError>> {
    let lowering = match name {
        "Table" => Some(lower_table_asset(state, object, ctx)),
        "Cupboard" => Some(lower_cupboard_asset(state, object, ctx)),
        "Lamp" => Some(lower_lamp_asset(state, object, ctx)),
        _ => None,
    }?;
    Some(instantiate_semantic_asset(
        ctx,
        read_transform(object),
        lowering,
    ))
}

fn transform_offset(transform: PrimitiveTransform, offset: Vec3) -> Vec3 {
    rotate_z(
        rotate_y(rotate_x(offset, transform.rot_deg.x), transform.rot_deg.y),
        transform.rot_deg.z,
    )
}

fn offset_transform(transform: PrimitiveTransform, offset: Vec3) -> PrimitiveTransform {
    PrimitiveTransform {
        center: transform.center.add(transform_offset(transform, offset)),
        rot_deg: transform.rot_deg,
    }
}

enum LoweredNodeSpec {
    Box {
        offset: Vec3,
        half_size: Vec3,
        round: f32,
        material_id: u32,
    },
    Cylinder {
        offset: Vec3,
        radius: f32,
        half_height: f32,
        round: f32,
        material_id: u32,
    },
    Sphere {
        offset: Vec3,
        radius: f32,
        material_id: u32,
    },
    Union(Vec<LoweredNodeSpec>),
    Subtract {
        lhs: Box<LoweredNodeSpec>,
        rhs: Box<LoweredNodeSpec>,
    },
}

fn instantiate_semantic_asset(
    ctx: &mut CompileContext,
    base_transform: PrimitiveTransform,
    spec: LoweredNodeSpec,
) -> Result<SdfNode, RenderError> {
    instantiate_lowered_node(ctx, base_transform, spec)
}

fn instantiate_lowered_node(
    ctx: &mut CompileContext,
    base_transform: PrimitiveTransform,
    spec: LoweredNodeSpec,
) -> Result<SdfNode, RenderError> {
    match spec {
        LoweredNodeSpec::Box {
            offset,
            half_size,
            round,
            material_id,
        } => {
            let transform = offset_transform(base_transform, offset);
            let object_id = ctx.alloc_object_id();
            ctx.register_object_transform(object_id, transform);
            Ok(SdfNode::Box {
                transform,
                half_size,
                round,
                shell: 0.0,
                object_id,
                material_id,
            })
        }
        LoweredNodeSpec::Cylinder {
            offset,
            radius,
            half_height,
            round,
            material_id,
        } => {
            let transform = offset_transform(base_transform, offset);
            let object_id = ctx.alloc_object_id();
            ctx.register_object_transform(object_id, transform);
            Ok(SdfNode::Cylinder {
                transform,
                radius,
                half_height,
                round,
                shell: 0.0,
                object_id,
                material_id,
            })
        }
        LoweredNodeSpec::Sphere {
            offset,
            radius,
            material_id,
        } => {
            let transform = offset_transform(base_transform, offset);
            let object_id = ctx.alloc_object_id();
            ctx.register_object_transform(object_id, transform);
            Ok(SdfNode::Sphere {
                transform,
                radius,
                shell: 0.0,
                object_id,
                material_id,
            })
        }
        LoweredNodeSpec::Union(parts) => {
            let mut parts = parts.into_iter();
            let Some(root) = parts.next() else {
                return Err(RenderError::ExpectedObject);
            };
            let mut root = instantiate_lowered_node(ctx, base_transform, root)?;
            for part in parts {
                root = SdfNode::Union {
                    lhs: Box::new(root),
                    rhs: Box::new(instantiate_lowered_node(ctx, base_transform, part)?),
                };
            }
            Ok(root)
        }
        LoweredNodeSpec::Subtract { lhs, rhs } => Ok(SdfNode::Subtract {
            lhs: Box::new(instantiate_lowered_node(ctx, base_transform, *lhs)?),
            rhs: Box::new(instantiate_lowered_node(ctx, base_transform, *rhs)?),
        }),
    }
}

fn lower_table_asset(
    state: &Arc<EvalState>,
    object: &ObjectValue,
    ctx: &mut CompileContext,
) -> LoweredNodeSpec {
    let width = read_number_field(object, &["width"])
        .unwrap_or(1.8)
        .max(0.2);
    let depth = read_number_field(object, &["depth"])
        .unwrap_or(0.9)
        .max(0.2);
    let height = read_number_field(object, &["height"])
        .unwrap_or(0.76)
        .max(0.1);
    let top_thickness = read_number_field(object, &["top_thickness"])
        .unwrap_or(0.08)
        .clamp(0.01, height.max(0.01));
    let leg_radius = read_number_field(object, &["leg_radius"])
        .unwrap_or(0.05)
        .max(0.005);
    let leg_inset = read_number_field(object, &["leg_inset"])
        .unwrap_or(0.12)
        .max(0.0);
    let top_material_id = object_material_id(state, object, "top_material", ctx);
    let leg_material_id = object_material_id(state, object, "leg_material", ctx);

    let mut parts = Vec::new();
    let top_center_y = height * 0.5 - top_thickness * 0.5;
    parts.push(LoweredNodeSpec::Box {
        offset: Vec3::new(0.0, top_center_y, 0.0),
        half_size: Vec3::new(width * 0.5, top_thickness * 0.5, depth * 0.5),
        round: 0.0,
        material_id: top_material_id,
    });

    let leg_half_height = ((height - top_thickness) * 0.5).max(0.001);
    let leg_center_y = -height * 0.5 + leg_half_height;
    let leg_x = (width * 0.5 - leg_inset).max(leg_radius);
    let leg_z = (depth * 0.5 - leg_inset).max(leg_radius);
    for &sx in &[-1.0_f32, 1.0] {
        for &sz in &[-1.0_f32, 1.0] {
            parts.push(LoweredNodeSpec::Cylinder {
                offset: Vec3::new(sx * leg_x, leg_center_y, sz * leg_z),
                radius: leg_radius,
                half_height: leg_half_height,
                round: 0.0,
                material_id: leg_material_id,
            });
        }
    }
    LoweredNodeSpec::Union(parts)
}

fn lower_cupboard_asset(
    state: &Arc<EvalState>,
    object: &ObjectValue,
    ctx: &mut CompileContext,
) -> LoweredNodeSpec {
    let width = read_number_field(object, &["width"])
        .unwrap_or(1.6)
        .max(0.1);
    let height = read_number_field(object, &["height"])
        .unwrap_or(2.0)
        .max(0.1);
    let depth = read_number_field(object, &["depth"])
        .unwrap_or(0.6)
        .max(0.1);
    let wall_thickness = read_number_field(object, &["wall_thickness"])
        .unwrap_or(0.05)
        .max(0.005);
    let open_amount = read_number_field(object, &["open_amount"])
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    let body_material_id = object_material_id(state, object, "body_material", ctx);
    let door_material_id = object_material_id(state, object, "door_material", ctx);

    let outer = LoweredNodeSpec::Box {
        offset: Vec3::new(0.0, 0.0, 0.0),
        half_size: Vec3::new(width * 0.5, height * 0.5, depth * 0.5),
        round: 0.0,
        material_id: body_material_id,
    };

    let inner_half = Vec3::new(
        (width * 0.5 - wall_thickness).max(0.001),
        (height * 0.5 - wall_thickness).max(0.001),
        (depth * 0.5 - wall_thickness).max(0.001),
    );
    let cavity = LoweredNodeSpec::Box {
        offset: Vec3::new(0.0, 0.0, -wall_thickness),
        half_size: inner_half,
        round: 0.0,
        material_id: body_material_id,
    };

    let shell = LoweredNodeSpec::Subtract {
        lhs: Box::new(outer),
        rhs: Box::new(cavity),
    };

    let door_thickness = (wall_thickness * (1.0 - open_amount)).max(0.001);
    let door = LoweredNodeSpec::Box {
        offset: Vec3::new(0.0, 0.0, depth * 0.5 - door_thickness * 0.5),
        half_size: Vec3::new(width * 0.5, height * 0.5, door_thickness * 0.5),
        round: 0.0,
        material_id: door_material_id,
    };

    LoweredNodeSpec::Union(vec![shell, door])
}

fn lower_lamp_asset(
    state: &Arc<EvalState>,
    object: &ObjectValue,
    ctx: &mut CompileContext,
) -> LoweredNodeSpec {
    let height = read_number_field(object, &["height"])
        .unwrap_or(0.72)
        .max(0.1);
    let base_radius = read_number_field(object, &["base_radius"])
        .unwrap_or(0.16)
        .max(0.02);
    let base_height = read_number_field(object, &["base_height"])
        .unwrap_or(0.05)
        .clamp(0.01, height.max(0.01));
    let stem_radius = read_number_field(object, &["stem_radius"])
        .unwrap_or(0.025)
        .max(0.005);
    let shade_radius = read_number_field(object, &["shade_radius"])
        .unwrap_or(0.2)
        .max(0.02);
    let shade_height = read_number_field(object, &["shade_height"])
        .unwrap_or(0.22)
        .clamp(0.02, height.max(0.02));
    let bulb_radius = read_number_field(object, &["bulb_radius"])
        .unwrap_or(0.065)
        .max(0.01);

    let base_material_id =
        object_material_id_with_fallback(state, object, "base_material", "body_material", ctx);
    let stem_material_id =
        object_material_id_with_fallback(state, object, "stem_material", "body_material", ctx);
    let shade_material_id =
        object_material_id_with_fallback(state, object, "shade_material", "body_material", ctx);
    let bulb_material_id = object_material_id(state, object, "bulb_material", ctx);
    let light_intensity = read_light_spectrum(
        object,
        "light_color",
        "light_intensity",
        &["light_intensity"],
    )
    .unwrap_or(Spectrum::rgb(18.0, 17.2, 15.6));
    let light_radius = read_number_field(object, &["light_radius"])
        .unwrap_or((bulb_radius * 0.85).max(0.02))
        .max(0.0);
    let light_samples = read_number_field(object, &["light_samples"])
        .map(|v| v.max(1.0) as u32)
        .unwrap_or(6);

    let base_center_y = -height * 0.5 + base_height * 0.5;
    let shade_center_y = height * 0.5 - shade_height * 0.5;
    let stem_half_height =
        ((shade_center_y - shade_height * 0.5) - (base_center_y + base_height * 0.5)).max(0.001)
            * 0.5;
    let stem_center_y =
        (shade_center_y - shade_height * 0.5 + base_center_y + base_height * 0.5) * 0.5;
    let bulb_center_y = shade_center_y - shade_height * 0.15;

    if light_intensity.r > 0.0 || light_intensity.g > 0.0 || light_intensity.b > 0.0 {
        ctx.semantic_lights.push(SemanticLight {
            position: read_transform(object).center.add(transform_offset(
                read_transform(object),
                Vec3::new(0.0, bulb_center_y, 0.0),
            )),
            radius: light_radius,
            intensity: light_intensity,
            samples: light_samples,
        });
    }

    LoweredNodeSpec::Union(vec![
        LoweredNodeSpec::Cylinder {
            offset: Vec3::new(0.0, base_center_y, 0.0),
            radius: base_radius,
            half_height: base_height * 0.5,
            round: 0.0,
            material_id: base_material_id,
        },
        LoweredNodeSpec::Cylinder {
            offset: Vec3::new(0.0, stem_center_y, 0.0),
            radius: stem_radius,
            half_height: stem_half_height,
            round: 0.0,
            material_id: stem_material_id,
        },
        LoweredNodeSpec::Cylinder {
            offset: Vec3::new(0.0, shade_center_y, 0.0),
            radius: shade_radius,
            half_height: shade_height * 0.5,
            round: 0.0,
            material_id: shade_material_id,
        },
        LoweredNodeSpec::Sphere {
            offset: Vec3::new(0.0, bulb_center_y, 0.0),
            radius: bulb_radius,
            material_id: bulb_material_id,
        },
    ])
}

fn compile_room(
    state: &Arc<EvalState>,
    object: &ObjectValue,
    ctx: &mut CompileContext,
) -> Result<SdfNode, RenderError> {
    let transform = read_transform(object);
    let width = read_number_field(object, &["width"])
        .unwrap_or(8.0)
        .max(0.1);
    let height = read_number_field(object, &["height"])
        .unwrap_or(4.0)
        .max(0.1);
    let depth = read_number_field(object, &["depth"])
        .unwrap_or(8.0)
        .max(0.1);
    let wall_thickness = read_number_field(object, &["wall_thickness"])
        .unwrap_or(0.18)
        .max(0.01);

    let show_floor = room_flag(object, "show_floor", true);
    let show_ceiling = room_flag(object, "show_ceiling", false);
    let show_back_wall = room_flag(object, "show_back_wall", true);
    let show_front_wall = room_flag(object, "show_front_wall", false);
    let show_left_wall = room_flag(object, "show_left_wall", false);
    let show_right_wall = room_flag(object, "show_right_wall", true);

    let floor_material_id = room_material_id(state, object, "floor_material", "wall_material", ctx);
    let ceiling_material_id =
        room_material_id(state, object, "ceiling_material", "wall_material", ctx);
    let back_material_id =
        room_material_id(state, object, "back_wall_material", "wall_material", ctx);
    let front_material_id =
        room_material_id(state, object, "front_wall_material", "wall_material", ctx);
    let left_material_id =
        room_material_id(state, object, "left_wall_material", "wall_material", ctx);
    let right_material_id =
        room_material_id(state, object, "right_wall_material", "wall_material", ctx);

    let mut parts = Vec::new();

    if show_floor {
        let object_id = ctx.alloc_object_id();
        let part_transform = PrimitiveTransform {
            center: transform
                .center
                .add(Vec3::new(0.0, -height * 0.5 + wall_thickness * 0.5, 0.0)),
            rot_deg: transform.rot_deg,
        };
        ctx.register_object_transform(object_id, part_transform);
        parts.push(SdfNode::Box {
            transform: part_transform,
            half_size: Vec3::new(width * 0.5, wall_thickness * 0.5, depth * 0.5),
            round: 0.0,
            shell: 0.0,
            object_id,
            material_id: floor_material_id,
        });
    }

    if show_ceiling {
        let object_id = ctx.alloc_object_id();
        let part_transform = PrimitiveTransform {
            center: transform
                .center
                .add(Vec3::new(0.0, height * 0.5 - wall_thickness * 0.5, 0.0)),
            rot_deg: transform.rot_deg,
        };
        ctx.register_object_transform(object_id, part_transform);
        parts.push(SdfNode::Box {
            transform: part_transform,
            half_size: Vec3::new(width * 0.5, wall_thickness * 0.5, depth * 0.5),
            round: 0.0,
            shell: 0.0,
            object_id,
            material_id: ceiling_material_id,
        });
    }

    if show_back_wall {
        let object_id = ctx.alloc_object_id();
        let part_transform = PrimitiveTransform {
            center: transform
                .center
                .add(Vec3::new(0.0, 0.0, -depth * 0.5 + wall_thickness * 0.5)),
            rot_deg: transform.rot_deg,
        };
        ctx.register_object_transform(object_id, part_transform);
        parts.push(SdfNode::Box {
            transform: part_transform,
            half_size: Vec3::new(width * 0.5, height * 0.5, wall_thickness * 0.5),
            round: 0.0,
            shell: 0.0,
            object_id,
            material_id: back_material_id,
        });
    }

    if show_front_wall {
        let object_id = ctx.alloc_object_id();
        let part_transform = PrimitiveTransform {
            center: transform
                .center
                .add(Vec3::new(0.0, 0.0, depth * 0.5 - wall_thickness * 0.5)),
            rot_deg: transform.rot_deg,
        };
        ctx.register_object_transform(object_id, part_transform);
        parts.push(SdfNode::Box {
            transform: part_transform,
            half_size: Vec3::new(width * 0.5, height * 0.5, wall_thickness * 0.5),
            round: 0.0,
            shell: 0.0,
            object_id,
            material_id: front_material_id,
        });
    }

    if show_left_wall {
        let object_id = ctx.alloc_object_id();
        let part_transform = PrimitiveTransform {
            center: transform
                .center
                .add(Vec3::new(-width * 0.5 + wall_thickness * 0.5, 0.0, 0.0)),
            rot_deg: transform.rot_deg,
        };
        ctx.register_object_transform(object_id, part_transform);
        parts.push(SdfNode::Box {
            transform: part_transform,
            half_size: Vec3::new(wall_thickness * 0.5, height * 0.5, depth * 0.5),
            round: 0.0,
            shell: 0.0,
            object_id,
            material_id: left_material_id,
        });
    }

    if show_right_wall {
        let object_id = ctx.alloc_object_id();
        let part_transform = PrimitiveTransform {
            center: transform
                .center
                .add(Vec3::new(width * 0.5 - wall_thickness * 0.5, 0.0, 0.0)),
            rot_deg: transform.rot_deg,
        };
        ctx.register_object_transform(object_id, part_transform);
        parts.push(SdfNode::Box {
            transform: part_transform,
            half_size: Vec3::new(wall_thickness * 0.5, height * 0.5, depth * 0.5),
            round: 0.0,
            shell: 0.0,
            object_id,
            material_id: right_material_id,
        });
    }

    let mut parts = parts.into_iter();
    let Some(mut root) = parts.next() else {
        let transform = read_transform(object);
        let object_id = ctx.alloc_object_id();
        ctx.register_object_transform(object_id, transform);
        let material_id = ctx.intern_material(ctx.default_material);
        return Ok(SdfNode::Box {
            transform,
            half_size: Vec3::new(0.001, 0.001, 0.001),
            round: 0.0,
            shell: 0.0,
            object_id,
            material_id,
        });
    };
    for part in parts {
        root = SdfNode::Union {
            lhs: Box::new(root),
            rhs: Box::new(part),
        };
    }
    Ok(root)
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
    render_depth_image(
        &accel,
        &setup,
        options,
        options.width.max(options.height),
        1,
    )
}

fn render_ray_with_accel_progressive<A: Accelerator + Sync + Send>(
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
    let aa_samples = settings.aa_samples.max(1);
    let sample_offsets = pixel_sample_offsets(aa_samples);
    let tile_size = settings.tile_size.max(1) as usize;
    let tiles = tile_jobs(width, height, tile_size);
    let tiles_total = tiles.len() as u32;
    let start = Instant::now();
    let mut tiles_done = 0_u32;
    let mut callback_error = None;
    let queue = Arc::new(Mutex::new(VecDeque::from(tiles)));
    let (tx, rx) = mpsc::channel::<TileResult>();
    let worker_count = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(tiles_total as usize)
        .max(1);
    thread::scope(|scope| {
        let accel_ref = &accel;
        let setup_ref = &setup;
        for _ in 0..worker_count {
            let queue = Arc::clone(&queue);
            let tx = tx.clone();
            let sample_offsets = sample_offsets.clone();
            scope.spawn(move || {
                loop {
                    let job = {
                        let mut queue = queue.lock().expect("tile queue lock should succeed");
                        queue.pop_front()
                    };
                    let Some(job) = job else {
                        break;
                    };
                    let data = render_ray_tile(
                        accel_ref,
                        setup_ref,
                        options,
                        ray_ctx,
                        settings.debug_aov,
                        aspect,
                        aa_samples,
                        &sample_offsets,
                        &job,
                    );
                    let _ = tx.send(TileResult { job, data });
                }
            });
        }
        drop(tx);

        for _ in 0..tiles_total {
            let tile = rx.recv().expect("worker tile result should arrive");
            merge_tile_into_buffer(&mut buffer, width, &tile.job, &tile.data);
            tiles_done += 1;
            let image = RgbImage::from_vec(options.width, options.height, buffer.clone())
                .expect("pixel buffer length must match image dimensions");
            if let Err(err) = on_tile(
                RayProgress {
                    tiles_done,
                    tiles_total,
                    elapsed_ms: start.elapsed().as_millis(),
                },
                &image,
            ) {
                callback_error = Some(err);
                break;
            }
        }
    });
    if let Some(err) = callback_error {
        return Err(err);
    }

    Ok(RgbImage::from_vec(options.width, options.height, buffer)
        .expect("pixel buffer length must match image dimensions"))
}

fn render_depth_image(
    accel: &(impl Accelerator + Sync),
    setup: &RenderSetup,
    options: RenderOptions,
    tile_size: u32,
    aa_samples: u32,
) -> RgbImage {
    render_preview_tiled(accel, setup, options, tile_size, aa_samples, &mut |_, _| {
        Ok(())
    })
    .expect("preview rendering without callback failure should succeed")
}

fn render_preview_with_accel_progressive<A: Accelerator + Sync + Send>(
    scene: CompiledScene,
    setup: RenderSetup,
    options: RenderOptions,
    tile_size: u32,
    aa_samples: u32,
    on_tile: &mut impl FnMut(PreviewProgress, &RgbImage) -> Result<(), RenderError>,
) -> Result<RgbImage, RenderError> {
    let accel = A::from_scene(scene);
    render_preview_tiled(&accel, &setup, options, tile_size, aa_samples, on_tile)
}

fn render_preview_tiled(
    accel: &(impl Accelerator + Sync),
    setup: &RenderSetup,
    options: RenderOptions,
    tile_size: u32,
    aa_samples: u32,
    on_tile: &mut impl FnMut(PreviewProgress, &RgbImage) -> Result<(), RenderError>,
) -> Result<RgbImage, RenderError> {
    let aspect = options.width as f32 / options.height as f32;
    let width = options.width as usize;
    let height = options.height as usize;
    let mut buffer = vec![0_u8; width * height * 3];
    let aa_samples = aa_samples.max(1);
    let sample_offsets = pixel_sample_offsets(aa_samples);
    let tile_size = tile_size.max(1) as usize;
    let tiles = tile_jobs(width, height, tile_size);
    let tiles_total = tiles.len() as u32;
    let start = Instant::now();
    let mut tiles_done = 0_u32;
    let mut callback_error = None;
    let queue = Arc::new(Mutex::new(VecDeque::from(tiles)));
    let (tx, rx) = mpsc::channel::<TileResult>();
    let worker_count = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(tiles_total as usize)
        .max(1);
    thread::scope(|scope| {
        let accel_ref = accel;
        let setup_ref = setup;
        for _ in 0..worker_count {
            let queue = Arc::clone(&queue);
            let tx = tx.clone();
            let sample_offsets = sample_offsets.clone();
            scope.spawn(move || {
                loop {
                    let job = {
                        let mut queue = queue.lock().expect("tile queue lock should succeed");
                        queue.pop_front()
                    };
                    let Some(job) = job else {
                        break;
                    };
                    let data = render_depth_tile(
                        accel_ref,
                        setup_ref,
                        options,
                        aspect,
                        aa_samples,
                        &sample_offsets,
                        &job,
                    );
                    let _ = tx.send(TileResult { job, data });
                }
            });
        }
        drop(tx);

        for _ in 0..tiles_total {
            let tile = rx.recv().expect("worker tile result should arrive");
            merge_tile_into_buffer(&mut buffer, width, &tile.job, &tile.data);
            tiles_done += 1;
            let image = RgbImage::from_vec(options.width, options.height, buffer.clone())
                .expect("pixel buffer length must match image dimensions");
            if let Err(err) = on_tile(
                PreviewProgress {
                    tiles_done,
                    tiles_total,
                    elapsed_ms: start.elapsed().as_millis(),
                },
                &image,
            ) {
                callback_error = Some(err);
                break;
            }
        }
    });
    if let Some(err) = callback_error {
        return Err(err);
    }

    Ok(RgbImage::from_vec(options.width, options.height, buffer)
        .expect("pixel buffer length must match image dimensions"))
}

#[derive(Clone)]
struct TileJob {
    tx: usize,
    ty: usize,
    tile_w: usize,
    tile_h: usize,
}

struct TileResult {
    job: TileJob,
    data: Vec<u8>,
}

fn tile_jobs(width: usize, height: usize, tile_size: usize) -> Vec<TileJob> {
    let mut jobs = Vec::new();
    for ty in (0..height).step_by(tile_size) {
        for tx in (0..width).step_by(tile_size) {
            jobs.push(TileJob {
                tx,
                ty,
                tile_w: (width - tx).min(tile_size),
                tile_h: (height - ty).min(tile_size),
            });
        }
    }
    jobs
}

fn merge_tile_into_buffer(buffer: &mut [u8], image_width: usize, job: &TileJob, tile: &[u8]) {
    for ly in 0..job.tile_h {
        let dst = ((job.ty + ly) * image_width + job.tx) * 3;
        let src = (ly * job.tile_w) * 3;
        let len = job.tile_w * 3;
        buffer[dst..dst + len].copy_from_slice(&tile[src..src + len]);
    }
}

#[allow(clippy::too_many_arguments)]
fn render_ray_tile(
    accel: &(impl Accelerator + Sync),
    setup: &RenderSetup,
    options: RenderOptions,
    ray_ctx: RayTraceCtx,
    debug_aov: Option<RayDebugAov>,
    aspect: f32,
    aa_samples: u32,
    sample_offsets: &[(f32, f32)],
    job: &TileJob,
) -> Vec<u8> {
    let mut tile = vec![0_u8; job.tile_w * job.tile_h * 3];
    tile.par_chunks_mut(job.tile_w * 3)
        .enumerate()
        .for_each(|(ly, row)| {
            let y = job.ty + ly;
            let y_u32 = y as u32;
            for lx in 0..job.tile_w {
                let x = job.tx + lx;
                let x_u32 = x as u32;
                let mut sum = Spectrum::black();
                for &(sx, sy) in sample_offsets {
                    let px = ((x_u32 as f32 + sx) / options.width as f32) * 2.0 - 1.0;
                    let py = 1.0 - ((y_u32 as f32 + sy) / options.height as f32) * 2.0;
                    let ray = setup.camera.generate_ray(px * aspect, py);
                    let origin = from_api_vec3(ray.origin);
                    let dir = from_api_vec3(ray.direction).normalize();
                    let c = if let Some(aov) = debug_aov {
                        ray::trace_ray_debug_aov(accel, setup, ray_ctx, origin, dir, aov)
                    } else {
                        ray::trace_ray_recursive(
                            accel,
                            setup,
                            ray_ctx,
                            origin,
                            dir,
                            MediumState::air(),
                            0,
                        )
                    };
                    sum = sum + c;
                }
                let avg = sum.scale(1.0 / aa_samples as f32);
                let rgb = if debug_aov.is_some() {
                    spectrum_to_rgb8(avg)
                } else {
                    spectrum_to_rgb8_reinhard(avg)
                };
                let i = lx * 3;
                row[i] = rgb[0];
                row[i + 1] = rgb[1];
                row[i + 2] = rgb[2];
            }
        });
    tile
}

fn render_depth_tile(
    accel: &(impl Accelerator + Sync),
    setup: &RenderSetup,
    options: RenderOptions,
    aspect: f32,
    aa_samples: u32,
    sample_offsets: &[(f32, f32)],
    job: &TileJob,
) -> Vec<u8> {
    let mut tile = vec![0_u8; job.tile_w * job.tile_h * 3];
    tile.par_chunks_mut(job.tile_w * 3)
        .enumerate()
        .for_each(|(ly, row)| {
            let y = job.ty + ly;
            let y_u32 = y as u32;
            for lx in 0..job.tile_w {
                let x = job.tx + lx;
                let x_u32 = x as u32;
                let mut sum = Spectrum::black();
                for &(sx, sy) in sample_offsets {
                    let px = ((x_u32 as f32 + sx) / options.width as f32) * 2.0 - 1.0;
                    let py = 1.0 - ((y_u32 as f32 + sy) / options.height as f32) * 2.0;
                    let ray = setup.camera.generate_ray(px * aspect, py);
                    let origin = from_api_vec3(ray.origin);
                    let dir = from_api_vec3(ray.direction).normalize();
                    let hit = raymarch_hit(accel, origin, dir, options, 0.0, options.max_dist);
                    let depth = match hit {
                        Some(hit) => depth_preview_value(hit.t, options.max_dist),
                        None => 0.0,
                    };
                    sum = sum + Spectrum::rgb(depth, depth, depth);
                }
                let rgb = spectrum_to_rgb8(sum.scale(1.0 / aa_samples as f32));
                let i = lx * 3;
                row[i] = rgb[0];
                row[i + 1] = rgb[1];
                row[i + 2] = rgb[2];
            }
        });
    tile
}

#[allow(dead_code)]
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
        SdfNode::ExtrudePolygon { transform, .. } => transform.center,
        SdfNode::Custom { transform, .. } => transform.center,
        SdfNode::DomainModifier { transform, .. }
        | SdfNode::DistancePostModifier { transform, .. } => transform.center,
        SdfNode::Union { lhs, rhs } => {
            let l = sdf_center(lhs);
            let r = sdf_center(rhs);
            Vec3::new((l.x + r.x) * 0.5, (l.y + r.y) * 0.5, (l.z + r.z) * 0.5)
        }
        SdfNode::Intersect { lhs, rhs }
        | SdfNode::UnionRound { lhs, rhs, .. }
        | SdfNode::UnionChamfer { lhs, rhs, .. }
        | SdfNode::UnionColumns { lhs, rhs, .. }
        | SdfNode::UnionStairs { lhs, rhs, .. }
        | SdfNode::UnionSoft { lhs, rhs, .. }
        | SdfNode::IntersectRound { lhs, rhs, .. }
        | SdfNode::IntersectChamfer { lhs, rhs, .. }
        | SdfNode::IntersectColumns { lhs, rhs, .. }
        | SdfNode::IntersectStairs { lhs, rhs, .. } => {
            let l = sdf_center(lhs);
            let r = sdf_center(rhs);
            Vec3::new((l.x + r.x) * 0.5, (l.y + r.y) * 0.5, (l.z + r.z) * 0.5)
        }
        SdfNode::Subtract { lhs, .. } => sdf_center(lhs),
        SdfNode::DiffRound { lhs, .. }
        | SdfNode::DiffChamfer { lhs, .. }
        | SdfNode::DiffColumns { lhs, .. }
        | SdfNode::DiffStairs { lhs, .. }
        | SdfNode::Pipe { lhs, .. }
        | SdfNode::Engrave { lhs, .. }
        | SdfNode::Groove { lhs, .. }
        | SdfNode::Tongue { lhs, .. } => sdf_center(lhs),
        SdfNode::Slice { base, .. } => sdf_center(base),
        SdfNode::Smooth { base, .. } => sdf_center(base),
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
    let (entry_t, exit_t) = ray_aabb_intersection(origin, dir, accel.scene_bounds())?;
    let max_t = max_t.min(exit_t);
    let mut traveled = min_t.max(entry_t.max(0.0));
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
        let step = (d.abs() * options.step_scale.clamp(0.05, 1.0))
            .max((options.epsilon * 0.5).max(1.0e-5));
        traveled += step;
    }
    None
}

fn ray_aabb_intersection(origin: Vec3, dir: Vec3, aabb: Aabb) -> Option<(f32, f32)> {
    let mut tmin = f32::NEG_INFINITY;
    let mut tmax = f32::INFINITY;

    for axis in 0..3 {
        let (o, d, min_v, max_v) = match axis {
            0 => (origin.x, dir.x, aabb.min.x, aabb.max.x),
            1 => (origin.y, dir.y, aabb.min.y, aabb.max.y),
            _ => (origin.z, dir.z, aabb.min.z, aabb.max.z),
        };

        if d.abs() <= 1.0e-8 {
            if o < min_v || o > max_v {
                return None;
            }
            continue;
        }

        let inv_d = 1.0 / d;
        let mut t0 = (min_v - o) * inv_d;
        let mut t1 = (max_v - o) * inv_d;
        if t0 > t1 {
            std::mem::swap(&mut t0, &mut t1);
        }
        tmin = tmin.max(t0);
        tmax = tmax.min(t1);
        if tmax < tmin {
            return None;
        }
    }

    Some((tmin, tmax))
}

fn secondary_min_t(epsilon: f32) -> f32 {
    // Hardwired higher for now to suppress immediate re-hits in tight reflective lips/rims.
    epsilon * 2.0
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
    let normal = Vec3::new(dx, dy, dz);
    if normal.length() > 1.0e-8 {
        normal.normalize()
    } else {
        Vec3::new(0.0, 1.0, 0.0)
    }
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
    let geometric_normal = if bsdf_ctx.hit.front_face {
        bsdf_ctx.hit.normal.normalize()
    } else {
        bsdf_ctx.hit.normal.mul(-1.0).normalize()
    };
    let mut color = Spectrum::black();
    for light in lights {
        let shadow_samples = light.shadow_sample_count().max(1);
        let inv_samples = 1.0 / shadow_samples as f32;
        for sample_index in 0..shadow_samples {
            let sample = light.sample_li_indexed(p, sample_index, shadow_samples);
            let wi = sample.wi.normalize();
            let ndotl = n.x * wi.x + n.y * wi.y + n.z * wi.z;
            if ndotl <= 0.0 {
                continue;
            }
            let shadow_origin = offset_ray_origin(
                bsdf_ctx.hit.position,
                geometric_normal,
                from_api_vec3(wi),
                options.epsilon,
            );
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
            color = color + (f * sample.radiance).scale(ndotl * shadow * inv_samples);
        }
    }
    color
}

#[allow(dead_code)]
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

fn environment_color(setup: &RenderSetup, dir: Vec3) -> Option<Spectrum> {
    let name = setup.environment_name.as_deref()?;
    let value =
        eval_environment_function(&setup.state, name, "color", &[vec3_value_value(dir)]).ok()?;
    spectrum_from_value(&value)
}

#[allow(dead_code)]
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

#[allow(dead_code)]
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
    let geometric_normal =
        resolve_surface_normal_at_hit(&setup.root, hit.position, hit.front_face, 1.0e-4)
            .unwrap_or_else(|| {
                if hit.front_face {
                    hit.normal.normalize()
                } else {
                    hit.normal.mul(-1.0).normalize()
                }
            });
    let material = material_for_id(&setup.materials, hit.material_id);
    let normal = resolve_dynamic_normal(
        &setup.state,
        &setup.material_def_names,
        &setup.dynamic_material_overrides,
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

fn resolve_surface_normal_at_hit(
    node: &SdfNode,
    p: Vec3,
    front_face: bool,
    epsilon: f32,
) -> Option<Vec3> {
    let n = resolve_surface_normal_from_node(node, p, epsilon)?;
    let n = if front_face { n } else { n.mul(-1.0) };
    Some(n.normalize())
}

fn resolve_surface_normal_from_node(node: &SdfNode, p: Vec3, epsilon: f32) -> Option<Vec3> {
    match node {
        SdfNode::Sphere { .. }
        | SdfNode::Box { .. }
        | SdfNode::Cylinder { .. }
        | SdfNode::Torus { .. }
        | SdfNode::ExtrudePolygon { .. }
        | SdfNode::Custom { .. } => Some(estimate_node_normal(node, p, epsilon)),
        SdfNode::DomainModifier { .. } | SdfNode::DistancePostModifier { .. } => {
            Some(estimate_node_normal(node, p, epsilon))
        }
        SdfNode::Union { lhs, rhs } => {
            let l = sdf_distance_info(lhs, p);
            let r = sdf_distance_info(rhs, p);
            if l.distance <= r.distance {
                resolve_surface_normal_from_node(lhs, p, epsilon)
            } else {
                resolve_surface_normal_from_node(rhs, p, epsilon)
            }
        }
        SdfNode::Intersect { lhs, rhs } => {
            let l = sdf_distance_info(lhs, p);
            let r = sdf_distance_info(rhs, p);
            if l.distance >= r.distance {
                resolve_surface_normal_from_node(lhs, p, epsilon)
            } else {
                resolve_surface_normal_from_node(rhs, p, epsilon)
            }
        }
        SdfNode::Subtract { lhs, rhs } => {
            let l = sdf_distance_info(lhs, p);
            let r = sdf_distance_info(rhs, p);
            if l.distance >= -r.distance {
                resolve_surface_normal_from_node(lhs, p, epsilon)
            } else {
                resolve_surface_normal_from_node(rhs, p, epsilon).map(|n| n.mul(-1.0))
            }
        }
        SdfNode::UnionRound { lhs, rhs, r }
        | SdfNode::UnionChamfer { lhs, rhs, r }
        | SdfNode::UnionSoft { lhs, rhs, r }
        | SdfNode::IntersectRound { lhs, rhs, r }
        | SdfNode::IntersectChamfer { lhs, rhs, r }
        | SdfNode::UnionColumns { lhs, rhs, r, .. }
        | SdfNode::UnionStairs { lhs, rhs, r, .. }
        | SdfNode::IntersectColumns { lhs, rhs, r, .. }
        | SdfNode::IntersectStairs { lhs, rhs, r, .. } => {
            blend_surface_normals(lhs, rhs, p, epsilon, *r, false)
        }
        SdfNode::DiffRound { lhs, rhs, r }
        | SdfNode::DiffChamfer { lhs, rhs, r }
        | SdfNode::DiffColumns { lhs, rhs, r, .. }
        | SdfNode::DiffStairs { lhs, rhs, r, .. }
        | SdfNode::Pipe { lhs, rhs, r }
        | SdfNode::Engrave { lhs, rhs, r } => blend_surface_normals(lhs, rhs, p, epsilon, *r, true),
        SdfNode::Groove { lhs, rhs, ra, rb } | SdfNode::Tongue { lhs, rhs, ra, rb } => {
            blend_surface_normals(lhs, rhs, p, epsilon, ra.max(*rb), true)
        }
        SdfNode::Slice { base, .. } => resolve_surface_normal_from_node(base, p, epsilon),
        SdfNode::Smooth { base, .. } => resolve_surface_normal_from_node(base, p, epsilon),
    }
}

fn blend_surface_normals(
    lhs: &SdfNode,
    rhs: &SdfNode,
    p: Vec3,
    epsilon: f32,
    radius: f32,
    difference_mode: bool,
) -> Option<Vec3> {
    let l = sdf_distance_info(lhs, p);
    let r = sdf_distance_info(rhs, p);
    let left = resolve_surface_normal_from_node(lhs, p, epsilon)?;
    let mut right = resolve_surface_normal_from_node(rhs, p, epsilon)?;
    let k = radius.abs().max(1.0e-6);
    let t = if difference_mode {
        right = right.mul(-1.0);
        smoothstepf(0.0, 1.0, 0.5 + 0.5 * (((-r.distance) - l.distance) / k))
    } else {
        smoothstepf(0.0, 1.0, 0.5 + 0.5 * ((l.distance - r.distance) / k))
    };
    let n = left.mul(1.0 - t).add(right.mul(t));
    if n.length() > 1.0e-6 {
        Some(n.normalize())
    } else if t < 0.5 {
        Some(left)
    } else {
        Some(right)
    }
}

fn estimate_node_normal(node: &SdfNode, p: Vec3, epsilon: f32) -> Vec3 {
    let e = epsilon.max(1.0e-5);
    let k1 = Vec3::new(1.0, -1.0, -1.0);
    let k2 = Vec3::new(-1.0, -1.0, 1.0);
    let k3 = Vec3::new(-1.0, 1.0, -1.0);
    let k4 = Vec3::new(1.0, 1.0, 1.0);
    let n = k1
        .mul(sdf_distance_info(node, p.add(k1.mul(e))).distance)
        .add(k2.mul(sdf_distance_info(node, p.add(k2.mul(e))).distance))
        .add(k3.mul(sdf_distance_info(node, p.add(k3.mul(e))).distance))
        .add(k4.mul(sdf_distance_info(node, p.add(k4.mul(e))).distance));
    if n.length() > 1.0e-6 {
        n.normalize()
    } else {
        Vec3::new(0.0, 1.0, 0.0)
    }
}

#[allow(dead_code)]
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

    let geometric_normal = if bsdf_ctx.hit.front_face {
        bsdf_ctx.hit.normal.normalize()
    } else {
        bsdf_ctx.hit.normal.mul(-1.0).normalize()
    };
    let shadow_origin = offset_ray_origin(hit_point, geometric_normal, li.wi, ctx.options.epsilon);
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
#[allow(dead_code)]
struct DirectLightSample {
    wi: Vec3,
    radiance: Spectrum,
    pdf: f32,
    max_t: f32,
    delta: bool,
}

#[allow(dead_code)]
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

#[allow(dead_code)]
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

#[allow(dead_code)]
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
    let mut t = secondary_min_t(epsilon);
    let mut visibility = 1.0_f32;
    for _ in 0..80 {
        if t >= max_t {
            return visibility.clamp(0.0, 1.0);
        }
        let p = origin.add(dir.mul(t));
        let lower = accel.lower_bound(p).abs();
        let h = if lower > (epsilon * 8.0).max(0.02) {
            lower
        } else {
            accel.distance(p).abs()
        };
        if h < epsilon * 4.0 {
            return 0.0;
        }
        visibility = visibility.min((10.0 * h / t).clamp(0.0, 1.0));
        t += h.max(epsilon * 2.0);
    }
    visibility.clamp(0.0, 1.0)
}

#[allow(dead_code)]
struct XorShift64 {
    state: u64,
}

#[allow(dead_code)]
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

#[allow(dead_code)]
fn seed_pixel(x: u32, y: u32, width: u32) -> u64 {
    let idx = u64::from(y) * u64::from(width) + u64::from(x);
    idx.wrapping_mul(0x9E3779B97F4A7C15)
        .wrapping_add(0xBF58476D1CE4E5B9)
}

#[allow(dead_code)]
fn seed_pixel_sample(x: u32, y: u32, width: u32, sample: u32) -> u64 {
    let base = seed_pixel(x, y, width);
    let s = u64::from(sample).wrapping_mul(0x94D0_49BB_1331_11EB);
    base ^ s ^ 0xD6E8_FEB8_6659_FD93
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
struct PixelAccumulator {
    sum: Spectrum,
    count: u32,
    mean_luma: f32,
    m2_luma: f32,
    active: bool,
}

#[allow(dead_code)]
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

#[allow(dead_code)]
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

fn pixel_sample_offsets(samples: u32) -> Vec<(f32, f32)> {
    let samples = samples.max(1);
    let grid = (samples as f32).sqrt().ceil() as u32;
    let mut offsets = Vec::with_capacity(samples as usize);
    for iy in 0..grid {
        for ix in 0..grid {
            if offsets.len() >= samples as usize {
                break;
            }
            offsets.push((
                (ix as f32 + 0.5) / grid as f32,
                (iy as f32 + 0.5) / grid as f32,
            ));
        }
    }
    offsets
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
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

#[allow(dead_code)]
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

#[allow(dead_code)]
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

#[allow(dead_code)]
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
    let overrides = dynamic_material_override(material, &setup.dynamic_material_overrides);
    let value =
        eval_material_function_with_overrides(&setup.state, name, function_name, ctx, overrides)
            .ok()?;
    spectrum_from_value(&value)
}

#[allow(dead_code)]
fn resolve_bsdf_number_hook(
    setup: &RenderSetup,
    material: MaterialKindRt,
    function_name: &str,
    ctx: Value,
) -> Option<f64> {
    let name = dynamic_material_name(material, &setup.material_def_names)?;
    let overrides = dynamic_material_override(material, &setup.dynamic_material_overrides);
    let value =
        eval_material_function_with_overrides(&setup.state, name, function_name, ctx, overrides)
            .ok()?;
    let Value::Number(v) = value else {
        return None;
    };
    Some(v)
}

#[allow(dead_code)]
fn resolve_bsdf_object_hook(
    setup: &RenderSetup,
    material: MaterialKindRt,
    function_name: &str,
    ctx: Value,
) -> Option<Value> {
    let name = dynamic_material_name(material, &setup.material_def_names)?;
    let overrides = dynamic_material_override(material, &setup.dynamic_material_overrides);
    eval_material_function_with_overrides(&setup.state, name, function_name, ctx, overrides).ok()
}

fn dynamic_material_name(material: MaterialKindRt, material_def_names: &[String]) -> Option<&str> {
    let dynamic_material_id = dominant_material_params(material).dynamic_material_id?;
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
        Value::String(_) | Value::Array(_) | Value::Function(_) => unreachable!(),
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

#[allow(dead_code)]
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

fn offset_ray_origin(position: Vec3, geometric_normal: Vec3, dir: Vec3, epsilon: f32) -> Vec3 {
    let n = if geometric_normal.length() > 1.0e-8 {
        geometric_normal.normalize()
    } else {
        dir.normalize()
    };
    let sign = if dir.dot(n) >= 0.0 { 1.0 } else { -1.0 };
    let normal_offset = n.mul(sign * (epsilon * 16.0).max(2.0e-4));
    let dir_step = (epsilon * 8.0).max(1.0e-4);
    let dir_offset = dir.normalize().mul(dir_step);
    position.add(normal_offset).add(dir_offset)
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

#[allow(dead_code)]
fn power_heuristic(pa: f32, pb: f32) -> f32 {
    let a2 = pa * pa;
    let b2 = pb * pb;
    a2 / (a2 + b2).max(1.0e-6)
}

#[allow(dead_code)]
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
    let params = dominant_material_params(material);
    let explicit = params.medium.map(|medium| MediumState {
        ior: medium.ior.clamp(1.0, 3.0),
        absorption_color: medium.absorption_color,
        density: medium.density.max(0.0),
    });
    explicit.or_else(|| {
        matches!(
            dominant_material_model(material),
            MaterialKindTag::Dielectric
        )
        .then_some(MediumState {
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
    if !matches!(
        dominant_material_model(material),
        MaterialKindTag::Dielectric
    ) {
        return current;
    }
    if front_face {
        medium_state_from_material(material).unwrap_or(current)
    } else {
        MediumState::air()
    }
}

#[allow(dead_code)]
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

fn depth_preview_value(t: f32, max_dist: f32) -> f32 {
    let d = (t / max_dist.max(1.0e-6)).clamp(0.0, 1.0);
    let shaped = 1.0 - d;
    shaped * shaped
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

#[allow(dead_code)]
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
        SdfNode::ExtrudePolygon {
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
        SdfNode::Custom {
            transform,
            bounds_half_extents,
            ..
        } => {
            let half = *bounds_half_extents;
            let corners = [
                Vec3::new(-half.x, -half.y, -half.z),
                Vec3::new(-half.x, -half.y, half.z),
                Vec3::new(-half.x, half.y, -half.z),
                Vec3::new(-half.x, half.y, half.z),
                Vec3::new(half.x, -half.y, -half.z),
                Vec3::new(half.x, -half.y, half.z),
                Vec3::new(half.x, half.y, -half.z),
                Vec3::new(half.x, half.y, half.z),
            ];
            let mut min = Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
            let mut max = Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
            for corner in corners {
                let world = transform.center.add(transform_offset(*transform, corner));
                min = Vec3::new(min.x.min(world.x), min.y.min(world.y), min.z.min(world.z));
                max = Vec3::new(max.x.max(world.x), max.y.max(world.y), max.z.max(world.z));
            }
            Aabb { min, max }
        }
        SdfNode::Union { lhs, rhs } => sdf_bounds(lhs).union(sdf_bounds(rhs)),
        SdfNode::Intersect { lhs, rhs } => sdf_bounds(lhs).union(sdf_bounds(rhs)),
        SdfNode::Subtract { lhs, .. } => sdf_bounds(lhs),
        SdfNode::UnionRound { lhs, rhs, .. }
        | SdfNode::UnionChamfer { lhs, rhs, .. }
        | SdfNode::UnionColumns { lhs, rhs, .. }
        | SdfNode::UnionStairs { lhs, rhs, .. }
        | SdfNode::UnionSoft { lhs, rhs, .. }
        | SdfNode::IntersectRound { lhs, rhs, .. }
        | SdfNode::IntersectChamfer { lhs, rhs, .. }
        | SdfNode::IntersectColumns { lhs, rhs, .. }
        | SdfNode::IntersectStairs { lhs, rhs, .. }
        | SdfNode::DiffRound { lhs, rhs, .. }
        | SdfNode::DiffChamfer { lhs, rhs, .. }
        | SdfNode::DiffColumns { lhs, rhs, .. }
        | SdfNode::DiffStairs { lhs, rhs, .. }
        | SdfNode::Pipe { lhs, rhs, .. }
        | SdfNode::Engrave { lhs, rhs, .. }
        | SdfNode::Groove { lhs, rhs, .. }
        | SdfNode::Tongue { lhs, rhs, .. } => sdf_bounds(lhs).union(sdf_bounds(rhs)),
        SdfNode::Slice {
            base,
            axis,
            min,
            max,
        } => {
            let mut bounds = sdf_bounds(base);
            match axis {
                0 => {
                    bounds.min.x = bounds.min.x.max(*min);
                    bounds.max.x = bounds.max.x.min(*max);
                    if bounds.max.x < bounds.min.x {
                        bounds.max.x = bounds.min.x;
                    }
                }
                1 => {
                    bounds.min.y = bounds.min.y.max(*min);
                    bounds.max.y = bounds.max.y.min(*max);
                    if bounds.max.y < bounds.min.y {
                        bounds.max.y = bounds.min.y;
                    }
                }
                _ => {
                    bounds.min.z = bounds.min.z.max(*min);
                    bounds.max.z = bounds.max.z.min(*max);
                    if bounds.max.z < bounds.min.z {
                        bounds.max.z = bounds.min.z;
                    }
                }
            }
            bounds
        }
        SdfNode::DomainModifier { bounds, .. } | SdfNode::DistancePostModifier { bounds, .. } => {
            *bounds
        }
        SdfNode::Smooth { base, k } => sdf_bounds(base).expand(*k * 0.1),
    }
}

fn point_aabb_lower_bound(p: Vec3, aabb: Aabb) -> f32 {
    let dx = if p.x < aabb.min.x {
        aabb.min.x - p.x
    } else if p.x > aabb.max.x {
        p.x - aabb.max.x
    } else {
        0.0
    };
    let dy = if p.y < aabb.min.y {
        aabb.min.y - p.y
    } else if p.y > aabb.max.y {
        p.y - aabb.max.y
    } else {
        0.0
    };
    let dz = if p.z < aabb.min.z {
        aabb.min.z - p.z
    } else if p.z > aabb.max.z {
        p.z - aabb.max.z
    } else {
        0.0
    };
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn sdf_lower_bound(node: &SdfNode, p: Vec3) -> f32 {
    match node {
        SdfNode::Sphere {
            transform,
            radius,
            shell,
            ..
        } => {
            let d = to_local(p, *transform).length() - *radius;
            if *shell > 0.0 {
                d.max(-(d + *shell))
            } else {
                d
            }
        }
        SdfNode::Box {
            transform,
            half_size,
            round,
            shell,
            ..
        } => {
            let inner_half = Vec3::new(
                (half_size.x - *round).max(0.0),
                (half_size.y - *round).max(0.0),
                (half_size.z - *round).max(0.0),
            );
            let q = to_local(p, *transform).abs().sub(inner_half);
            let outside = Vec3::new(q.x.max(0.0), q.y.max(0.0), q.z.max(0.0)).length();
            let inside = q.x.max(q.y).max(q.z).min(0.0);
            let d = outside + inside - *round;
            if *shell > 0.0 {
                d.max(-(d + *shell))
            } else {
                d
            }
        }
        SdfNode::Cylinder {
            transform,
            radius,
            half_height,
            round,
            shell,
            ..
        } => {
            let q = to_local(p, *transform);
            let radial = (q.x * q.x + q.z * q.z).sqrt();
            let dx = radial - (*radius - *round).max(0.0);
            let dy = q.y.abs() - (*half_height - *round).max(0.0);
            let outside = (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt();
            let inside = dx.max(dy).min(0.0);
            let d = outside + inside - *round;
            if *shell > 0.0 {
                d.max(-(d + *shell))
            } else {
                d
            }
        }
        SdfNode::Torus {
            transform,
            major_radius,
            minor_radius,
            ..
        } => {
            let q = to_local(p, *transform);
            let qx = (q.x * q.x + q.z * q.z).sqrt() - *major_radius;
            (qx * qx + q.y * q.y).sqrt() - *minor_radius
        }
        SdfNode::ExtrudePolygon {
            transform,
            sides,
            radius,
            half_height,
            round,
            shell,
            ..
        } => {
            let q = to_local(p, *transform);
            let radial = sd_regular_ngon(
                Vec3::new(q.x, 0.0, q.z),
                *sides,
                (*radius - *round).max(0.0),
            );
            let dy = q.y.abs() - (*half_height - *round).max(0.0);
            let outside = (radial.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt();
            let inside = radial.max(dy).min(0.0);
            let d = outside + inside - *round;
            if *shell > 0.0 {
                d.max(-(d + *shell))
            } else {
                d
            }
        }
        SdfNode::Custom {
            transform,
            bounds_half_extents,
            ..
        } => point_aabb_lower_bound(
            to_local(p, *transform),
            Aabb {
                min: bounds_half_extents.mul(-1.0),
                max: *bounds_half_extents,
            },
        ),
        SdfNode::Union { lhs, rhs } => sdf_lower_bound(lhs, p).min(sdf_lower_bound(rhs, p)),
        SdfNode::Intersect { lhs, rhs } => sdf_lower_bound(lhs, p).max(sdf_lower_bound(rhs, p)),
        SdfNode::Subtract { lhs, .. } => sdf_lower_bound(lhs, p),
        SdfNode::UnionRound { .. }
        | SdfNode::UnionChamfer { .. }
        | SdfNode::UnionColumns { .. }
        | SdfNode::UnionStairs { .. }
        | SdfNode::UnionSoft { .. }
        | SdfNode::IntersectRound { .. }
        | SdfNode::IntersectChamfer { .. }
        | SdfNode::IntersectColumns { .. }
        | SdfNode::IntersectStairs { .. }
        | SdfNode::DiffRound { .. }
        | SdfNode::DiffChamfer { .. }
        | SdfNode::DiffColumns { .. }
        | SdfNode::DiffStairs { .. }
        | SdfNode::Pipe { .. }
        | SdfNode::Engrave { .. }
        | SdfNode::Groove { .. }
        | SdfNode::Tongue { .. }
        | SdfNode::Slice { .. }
        | SdfNode::DomainModifier { .. }
        | SdfNode::DistancePostModifier { .. } => point_aabb_lower_bound(p, sdf_bounds(node)),
        SdfNode::Smooth { base, k } => sdf_lower_bound(base, p) - *k * 0.1,
    }
}

fn sdf_distance_info(node: &SdfNode, p: Vec3) -> DistanceInfo {
    match node {
        SdfNode::Sphere {
            transform,
            radius,
            shell,
            object_id,
            material_id,
        } => {
            let q = to_local(p, *transform);
            let d = q.length() - *radius;
            DistanceInfo {
                distance: if *shell > 0.0 {
                    d.max(-(d + *shell))
                } else {
                    d
                },
                object_id: *object_id,
                material_id: *material_id,
            }
        }
        SdfNode::Box {
            transform,
            half_size,
            round,
            shell,
            object_id,
            material_id,
        } => {
            let inner_half = Vec3::new(
                (half_size.x - *round).max(0.0),
                (half_size.y - *round).max(0.0),
                (half_size.z - *round).max(0.0),
            );
            let q = to_local(p, *transform).abs().sub(inner_half);
            let outside = Vec3::new(q.x.max(0.0), q.y.max(0.0), q.z.max(0.0)).length();
            let inside = q.x.max(q.y).max(q.z).min(0.0);
            let d = outside + inside - *round;
            DistanceInfo {
                distance: if *shell > 0.0 {
                    d.max(-(d + *shell))
                } else {
                    d
                },
                object_id: *object_id,
                material_id: *material_id,
            }
        }
        SdfNode::Cylinder {
            transform,
            radius,
            half_height,
            round,
            shell,
            object_id,
            material_id,
        } => {
            let q = to_local(p, *transform);
            let radial = (q.x * q.x + q.z * q.z).sqrt();
            let dx = radial - (*radius - *round).max(0.0);
            let dy = q.y.abs() - (*half_height - *round).max(0.0);
            let outside = (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt();
            let inside = dx.max(dy).min(0.0);
            let d = outside + inside - *round;
            DistanceInfo {
                distance: if *shell > 0.0 {
                    d.max(-(d + *shell))
                } else {
                    d
                },
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
        SdfNode::ExtrudePolygon {
            transform,
            sides,
            radius,
            half_height,
            round,
            shell,
            object_id,
            material_id,
        } => {
            let q = to_local(p, *transform);
            let radial = sd_regular_ngon(
                Vec3::new(q.x, 0.0, q.z),
                *sides,
                (*radius - *round).max(0.0),
            );
            let dy = q.y.abs() - (*half_height - *round).max(0.0);
            let outside = (radial.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt();
            let inside = radial.max(dy).min(0.0);
            let d = outside + inside - *round;
            DistanceInfo {
                distance: if *shell > 0.0 {
                    d.max(-(d + *shell))
                } else {
                    d
                },
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
        SdfNode::DomainModifier {
            base,
            runtime,
            transform,
            ..
        } => {
            let q = to_local(p, *transform);
            sdf_distance_info(base, eval_modifier_domain(runtime, q))
        }
        SdfNode::DistancePostModifier {
            base,
            runtime,
            transform,
            ..
        } => {
            let q = to_local(p, *transform);
            let mut info = sdf_distance_info(base, q);
            info.distance = eval_modifier_distance_post(runtime, info.distance, q);
            info
        }
        SdfNode::Union { lhs, rhs } => {
            let lhs_lb = sdf_lower_bound(lhs, p);
            let rhs_lb = sdf_lower_bound(rhs, p);
            if lhs_lb <= rhs_lb {
                let l = sdf_distance_info(lhs, p);
                if l.distance <= rhs_lb {
                    l
                } else {
                    let r = sdf_distance_info(rhs, p);
                    if l.distance <= r.distance { l } else { r }
                }
            } else {
                let r = sdf_distance_info(rhs, p);
                if r.distance <= lhs_lb {
                    r
                } else {
                    let l = sdf_distance_info(lhs, p);
                    if l.distance <= r.distance { l } else { r }
                }
            }
        }
        SdfNode::Intersect { lhs, rhs } => {
            let l = sdf_distance_info(lhs, p);
            let r = sdf_distance_info(rhs, p);
            if l.distance >= r.distance { l } else { r }
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
        SdfNode::UnionRound { lhs, rhs, r } => {
            let l = sdf_distance_info(lhs, p);
            let r_info = sdf_distance_info(rhs, p);
            let distance = op_union_round(l.distance, r_info.distance, *r);
            if l.distance <= r_info.distance {
                DistanceInfo { distance, ..l }
            } else {
                DistanceInfo { distance, ..r_info }
            }
        }
        SdfNode::UnionChamfer { lhs, rhs, r } => {
            let l = sdf_distance_info(lhs, p);
            let r_info = sdf_distance_info(rhs, p);
            let distance = op_union_chamfer(l.distance, r_info.distance, *r);
            if l.distance <= r_info.distance {
                DistanceInfo { distance, ..l }
            } else {
                DistanceInfo { distance, ..r_info }
            }
        }
        SdfNode::UnionColumns { lhs, rhs, r, n } => {
            let l = sdf_distance_info(lhs, p);
            let r_info = sdf_distance_info(rhs, p);
            let distance = op_union_columns(l.distance, r_info.distance, *r, *n);
            if l.distance <= r_info.distance {
                DistanceInfo { distance, ..l }
            } else {
                DistanceInfo { distance, ..r_info }
            }
        }
        SdfNode::UnionStairs { lhs, rhs, r, n } => {
            let l = sdf_distance_info(lhs, p);
            let r_info = sdf_distance_info(rhs, p);
            let distance = op_union_stairs(l.distance, r_info.distance, *r, *n);
            if l.distance <= r_info.distance {
                DistanceInfo { distance, ..l }
            } else {
                DistanceInfo { distance, ..r_info }
            }
        }
        SdfNode::UnionSoft { lhs, rhs, r } => {
            let l = sdf_distance_info(lhs, p);
            let r_info = sdf_distance_info(rhs, p);
            let distance = op_union_soft(l.distance, r_info.distance, *r);
            if l.distance <= r_info.distance {
                DistanceInfo { distance, ..l }
            } else {
                DistanceInfo { distance, ..r_info }
            }
        }
        SdfNode::IntersectRound { lhs, rhs, r } => {
            let l = sdf_distance_info(lhs, p);
            let r_info = sdf_distance_info(rhs, p);
            let distance = op_intersect_round(l.distance, r_info.distance, *r);
            if l.distance >= r_info.distance {
                DistanceInfo { distance, ..l }
            } else {
                DistanceInfo { distance, ..r_info }
            }
        }
        SdfNode::IntersectChamfer { lhs, rhs, r } => {
            let l = sdf_distance_info(lhs, p);
            let r_info = sdf_distance_info(rhs, p);
            let distance = op_intersect_chamfer(l.distance, r_info.distance, *r);
            if l.distance >= r_info.distance {
                DistanceInfo { distance, ..l }
            } else {
                DistanceInfo { distance, ..r_info }
            }
        }
        SdfNode::IntersectColumns { lhs, rhs, r, n } => {
            let l = sdf_distance_info(lhs, p);
            let r_info = sdf_distance_info(rhs, p);
            let distance = op_intersect_columns(l.distance, r_info.distance, *r, *n);
            if l.distance >= r_info.distance {
                DistanceInfo { distance, ..l }
            } else {
                DistanceInfo { distance, ..r_info }
            }
        }
        SdfNode::IntersectStairs { lhs, rhs, r, n } => {
            let l = sdf_distance_info(lhs, p);
            let r_info = sdf_distance_info(rhs, p);
            let distance = op_intersect_stairs(l.distance, r_info.distance, *r, *n);
            if l.distance >= r_info.distance {
                DistanceInfo { distance, ..l }
            } else {
                DistanceInfo { distance, ..r_info }
            }
        }
        SdfNode::DiffRound { lhs, rhs, r } => {
            let l = sdf_distance_info(lhs, p);
            let r_info = sdf_distance_info(rhs, p);
            let rd = -r_info.distance;
            let distance = op_diff_round(l.distance, r_info.distance, *r);
            if l.distance >= rd {
                DistanceInfo { distance, ..l }
            } else {
                DistanceInfo {
                    distance,
                    object_id: r_info.object_id,
                    material_id: r_info.material_id,
                }
            }
        }
        SdfNode::DiffChamfer { lhs, rhs, r } => {
            let l = sdf_distance_info(lhs, p);
            let r_info = sdf_distance_info(rhs, p);
            let rd = -r_info.distance;
            let distance = op_diff_chamfer(l.distance, r_info.distance, *r);
            if l.distance >= rd {
                DistanceInfo { distance, ..l }
            } else {
                DistanceInfo {
                    distance,
                    object_id: r_info.object_id,
                    material_id: r_info.material_id,
                }
            }
        }
        SdfNode::DiffColumns { lhs, rhs, r, n } => {
            let l = sdf_distance_info(lhs, p);
            let r_info = sdf_distance_info(rhs, p);
            let rd = -r_info.distance;
            let distance = op_diff_columns(l.distance, r_info.distance, *r, *n);
            if l.distance >= rd {
                DistanceInfo { distance, ..l }
            } else {
                DistanceInfo {
                    distance,
                    object_id: r_info.object_id,
                    material_id: r_info.material_id,
                }
            }
        }
        SdfNode::DiffStairs { lhs, rhs, r, n } => {
            let l = sdf_distance_info(lhs, p);
            let r_info = sdf_distance_info(rhs, p);
            let rd = -r_info.distance;
            let distance = op_diff_stairs(l.distance, r_info.distance, *r, *n);
            if l.distance >= rd {
                DistanceInfo { distance, ..l }
            } else {
                DistanceInfo {
                    distance,
                    object_id: r_info.object_id,
                    material_id: r_info.material_id,
                }
            }
        }
        SdfNode::Pipe { lhs, rhs, r } => {
            let l = sdf_distance_info(lhs, p);
            let r_info = sdf_distance_info(rhs, p);
            DistanceInfo {
                distance: op_pipe(l.distance, r_info.distance, *r),
                ..l
            }
        }
        SdfNode::Engrave { lhs, rhs, r } => {
            let l = sdf_distance_info(lhs, p);
            let r_info = sdf_distance_info(rhs, p);
            DistanceInfo {
                distance: op_engrave(l.distance, r_info.distance, *r),
                ..l
            }
        }
        SdfNode::Groove { lhs, rhs, ra, rb } => {
            let l = sdf_distance_info(lhs, p);
            let r_info = sdf_distance_info(rhs, p);
            DistanceInfo {
                distance: op_groove(l.distance, r_info.distance, *ra, *rb),
                ..l
            }
        }
        SdfNode::Tongue { lhs, rhs, ra, rb } => {
            let l = sdf_distance_info(lhs, p);
            let r_info = sdf_distance_info(rhs, p);
            DistanceInfo {
                distance: op_tongue(l.distance, r_info.distance, *ra, *rb),
                ..l
            }
        }
        SdfNode::Slice {
            base,
            axis,
            min,
            max,
        } => {
            let mut info = sdf_distance_info(base, p);
            let coord = match axis {
                0 => p.x,
                1 => p.y,
                _ => p.z,
            };
            let slab = (min - coord).max(coord - max);
            info.distance = info.distance.max(slab);
            info
        }
        SdfNode::Smooth { base, k } => {
            let mut info = sdf_distance_info(base, p);
            info.distance -= *k * 0.1;
            info
        }
    }
}

fn op_union_round(a: f32, b: f32, r: f32) -> f32 {
    let r = r.abs().max(1.0e-6);
    let ux = (r - a).max(0.0);
    let uy = (r - b).max(0.0);
    r.max(a.min(b)) - (ux * ux + uy * uy).sqrt()
}

fn op_intersect_round(a: f32, b: f32, r: f32) -> f32 {
    let r = r.abs().max(1.0e-6);
    let ux = (r + a).max(0.0);
    let uy = (r + b).max(0.0);
    (-r).min(a.max(b)) + (ux * ux + uy * uy).sqrt()
}

fn op_diff_round(a: f32, b: f32, r: f32) -> f32 {
    op_intersect_round(a, -b, r)
}

fn op_union_chamfer(a: f32, b: f32, r: f32) -> f32 {
    a.min(b).min((a - r + b) * std::f32::consts::FRAC_1_SQRT_2)
}

fn op_intersect_chamfer(a: f32, b: f32, r: f32) -> f32 {
    a.max(b).max((a + r + b) * std::f32::consts::FRAC_1_SQRT_2)
}

fn op_diff_chamfer(a: f32, b: f32, r: f32) -> f32 {
    op_intersect_chamfer(a, -b, r)
}

fn repeat_centered(mut p: f32, size: f32) -> f32 {
    let size = size.max(1.0e-6);
    p = (p + size * 0.5).rem_euclid(size) - size * 0.5;
    p
}

fn rotate45(x: f32, y: f32) -> (f32, f32) {
    (
        (x + y) * std::f32::consts::FRAC_1_SQRT_2,
        (y - x) * std::f32::consts::FRAC_1_SQRT_2,
    )
}

fn op_union_columns(a: f32, b: f32, r: f32, n: f32) -> f32 {
    if a < r && b < r {
        let n = n.round().max(2.0);
        let column_radius = r * (2.0_f32).sqrt() / ((n - 1.0) * 2.0 + (2.0_f32).sqrt());
        let mut x = a + column_radius;
        let mut y = b + column_radius;
        if (n as i32) % 2 == 1 {
            x += column_radius;
        }
        (x, y) = rotate45(x, y);
        x -= (2.0_f32).sqrt() * 0.5 * r;
        x += -column_radius * (2.0_f32).sqrt();
        if (n as i32) % 2 == 1 {
            y += column_radius;
        }
        y = repeat_centered(y, column_radius * 2.0);
        return (x * x + y * y).sqrt() - column_radius;
    }
    a.min(b)
}

fn op_intersect_columns(a: f32, b: f32, r: f32, n: f32) -> f32 {
    -op_union_columns(-a, -b, r, n)
}

fn op_diff_columns(a: f32, b: f32, r: f32, n: f32) -> f32 {
    -op_union_columns(-a, b, r, n)
}

fn op_union_stairs(a: f32, b: f32, r: f32, n: f32) -> f32 {
    let n = n.round().max(1.0);
    let s = r / n;
    let u = b - r;
    a.min(b)
        .min(0.5 * (u + a + ((u - a + s).rem_euclid(2.0 * s) - s).abs()))
}

fn op_intersect_stairs(a: f32, b: f32, r: f32, n: f32) -> f32 {
    -op_union_stairs(-a, -b, r, n)
}

fn op_diff_stairs(a: f32, b: f32, r: f32, n: f32) -> f32 {
    -op_union_stairs(-a, b, r, n)
}

fn op_union_soft(a: f32, b: f32, r: f32) -> f32 {
    let r = r.abs().max(1.0e-6);
    let e = (r - (a - b).abs()).max(0.0);
    a.min(b) - e * e * 0.25 / r
}

fn op_pipe(a: f32, b: f32, r: f32) -> f32 {
    (a * a + b * b).sqrt() - r.abs()
}

fn op_engrave(a: f32, b: f32, r: f32) -> f32 {
    a.max((a + r.abs() - b.abs()) * std::f32::consts::FRAC_1_SQRT_2)
}

fn op_groove(a: f32, b: f32, ra: f32, rb: f32) -> f32 {
    a.max((a + ra.abs()).min(rb.abs() - b.abs()))
}

fn op_tongue(a: f32, b: f32, ra: f32, rb: f32) -> f32 {
    a.min((a - ra.abs()).max(b.abs() - rb.abs()))
}

fn eval_custom_sdf_distance(runtime: &CustomSdfRuntime, p: Vec3) -> f32 {
    let domain_value = eval_sdf_vec3_function_with_overrides(
        &runtime.state,
        &runtime.name,
        "domain",
        vec3_value_value(p),
        Some(&runtime.overrides),
    )
    .ok();
    let domain_p = match domain_value {
        Some(Value::Object(obj)) => {
            let x = read_number_field(&obj, &["x"]).unwrap_or(p.x);
            let y = read_number_field(&obj, &["y"]).unwrap_or(p.y);
            let z = read_number_field(&obj, &["z"]).unwrap_or(p.z);
            Vec3::new(x, y, z)
        }
        _ => p,
    };
    let value = eval_sdf_function_with_overrides(
        &runtime.state,
        &runtime.name,
        "distance",
        vec3_value_value(domain_p),
        Some(&runtime.overrides),
    );
    let distance = match value {
        Ok(Value::Number(v)) => v as f32,
        _ => 1.0e6,
    };
    let post = eval_sdf_function_args_with_overrides(
        &runtime.state,
        &runtime.name,
        "distance_post",
        vec![Value::Number(distance as f64), vec3_value_value(domain_p)],
        Some(&runtime.overrides),
    );
    match post {
        Ok(Value::Number(v)) => v as f32,
        _ => distance,
    }
}

fn eval_custom_sdf_bounds_half_extents(
    state: &EvalState,
    name: &str,
    overrides: &ObjectValue,
) -> Vec3 {
    let value = eval_sdf_zero_arg_function_with_overrides(state, name, "bounds", Some(overrides));
    match value {
        Ok(Value::Object(obj)) => {
            let x = read_number_field(&obj, &["x"]).unwrap_or(0.0);
            let y = read_number_field(&obj, &["y"]).unwrap_or(0.0);
            let z = read_number_field(&obj, &["z"]).unwrap_or(0.0);
            Vec3::new(
                x.abs().max(1.0e-3),
                y.abs().max(1.0e-3),
                z.abs().max(1.0e-3),
            )
        }
        Ok(Value::Number(radius)) => {
            let r = (radius as f32).abs().max(1.0e-3);
            Vec3::new(r, r, r)
        }
        _ => Vec3::new(10_000.0, 10_000.0, 10_000.0),
    }
}

fn sd_regular_ngon(p: Vec3, sides: u32, radius: f32) -> f32 {
    let n = sides.max(3) as f32;
    let an = std::f32::consts::PI / n;
    let apothem = radius * an.cos();
    let angle = p.z.atan2(p.x);
    let sector = 2.0 * an;
    let wrapped = ((angle + an).rem_euclid(sector)) - an;
    p.x.hypot(p.z) * wrapped.cos() - apothem
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
    let (lights, path_lights) = parse_lights(state, &scene.semantic_lights);
    RenderSetup {
        state: state.clone(),
        root: scene.root.clone(),
        camera,
        lights,
        path_lights,
        materials: scene.materials.clone(),
        object_transforms: scene.object_transforms.clone(),
        material_def_names: sorted_material_def_names(state),
        dynamic_material_overrides: scene.dynamic_material_overrides.clone(),
        environment_name: find_environment_name(state),
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

fn parse_lights(
    state: &EvalState,
    semantic_lights: &[SemanticLight],
) -> (Vec<Box<dyn Light>>, Vec<PathLight>) {
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
                let intensity = read_light_spectrum(obj, "color", "intensity", &["intensity"])
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
                let radiance =
                    read_light_spectrum(obj, "color", "intensity", &["radiance", "color"])
                        .unwrap_or(Spectrum::rgb(0.15, 0.15, 0.15));
                lights.push(Box::new(EnvLight { radiance }));
                path_lights.push(PathLight::Env { radiance });
            }
            "SphereLight" => {
                let position = read_vec3_field(obj, "position").unwrap_or_else(|| read_center(obj));
                let radius = read_number_field(obj, &["radius", "r"])
                    .unwrap_or(0.35)
                    .max(0.0);
                let intensity = read_light_spectrum(obj, "color", "intensity", &["intensity"])
                    .unwrap_or(Spectrum::rgb(8.0, 8.0, 8.0));
                let samples = read_number_field(obj, &["samples"])
                    .map(|v| v.max(1.0) as u32)
                    .unwrap_or(8);
                lights.push(Box::new(SphereLight {
                    position: to_api_vec3(position),
                    radius,
                    intensity,
                    samples,
                }));
                path_lights.push(PathLight::Point {
                    position: to_api_vec3(position),
                    intensity,
                });
            }
            _ => {}
        }
    }

    for light in semantic_lights {
        lights.push(Box::new(SphereLight {
            position: to_api_vec3(light.position),
            radius: light.radius,
            intensity: light.intensity,
            samples: light.samples,
        }));
        path_lights.push(PathLight::Point {
            position: to_api_vec3(light.position),
            intensity: light.intensity,
        });
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
        return Some(material_from_object(state, material_obj, None));
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
        return Some(material_from_object(state, obj, None));
    }

    if state.material_defs.contains_key(type_name) {
        return Some(material_from_dynamic_def(state, type_name, obj, None));
    }

    None
}

fn material_from_object(
    state: &EvalState,
    obj: &ObjectValue,
    ctx: Option<&mut CompileContext>,
) -> MaterialKindRt {
    let type_name = obj.type_name.as_deref().unwrap_or_default();
    if state.material_defs.contains_key(type_name) {
        return material_from_dynamic_def(state, type_name, obj, ctx);
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

fn material_from_dynamic_def(
    state: &EvalState,
    name: &str,
    overrides: &ObjectValue,
    ctx: Option<&mut CompileContext>,
) -> MaterialKindRt {
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
    apply_dynamic_material_properties(state, name, overrides, &mut params);
    params.dynamic_material_id = sorted_material_def_names(state)
        .iter()
        .position(|candidate| candidate == name)
        .map(|idx| idx as u32);
    params.dynamic_override_id =
        ctx.and_then(|ctx| ctx.intern_dynamic_material_override(overrides));
    match def.model.as_str() {
        "Metal" => MaterialKindRt::Metal(params),
        "Dielectric" => MaterialKindRt::Dielectric(params),
        _ => MaterialKindRt::Lambert(params),
    }
}

fn apply_dynamic_material_properties(
    state: &EvalState,
    material_name: &str,
    overrides: &ObjectValue,
    params: &mut MaterialParams,
) {
    let Ok(properties) =
        eval_material_properties_with_overrides(state, material_name, Some(overrides))
    else {
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
        Value::String(_) | Value::Array(_) | Value::Function(_) => None,
    }
}

fn read_light_spectrum(
    obj: &ObjectValue,
    color_field: &str,
    scalar_intensity_field: &str,
    legacy_fields: &[&str],
) -> Option<Spectrum> {
    if let Some(color) = read_spectrum_field(obj, color_field) {
        let intensity = read_number_field(obj, &[scalar_intensity_field]).unwrap_or(1.0);
        return Some(color.scale(intensity.max(0.0)));
    }

    legacy_fields
        .iter()
        .find_map(|field| read_spectrum_field(obj, field))
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
        Value::String(_) | Value::Array(_) | Value::Function(_) => None,
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

fn find_environment_name(state: &EvalState) -> Option<String> {
    for key in ["environment", "env", "sky"] {
        if let Some(binding) = state.bindings.get(key)
            && let Value::Object(obj) = &binding.value
            && let Some(type_name) = obj.type_name.as_deref()
            && state.environment_defs.contains_key(type_name)
        {
            return Some(type_name.to_string());
        }
    }

    if state.environment_defs.len() == 1 {
        return state.environment_defs.keys().next().cloned();
    }

    None
}

fn resolve_material_at_hit(setup: &RenderSetup, hit: RayHit, view_dir: Vec3) -> MaterialKindRt {
    resolve_material_from_node(&setup.root, setup, hit, view_dir).unwrap_or_else(|| {
        resolve_leaf_material(
            material_for_id(&setup.materials, hit.material_id),
            setup,
            hit,
            view_dir,
        )
    })
}

fn resolve_split_material_at_hit(
    setup: &RenderSetup,
    hit: RayHit,
    view_dir: Vec3,
) -> Option<(MaterialKindRt, MaterialKindRt, f32)> {
    resolve_split_material_from_node(&setup.root, setup, hit, view_dir)
}

fn resolve_material_from_node(
    node: &SdfNode,
    setup: &RenderSetup,
    hit: RayHit,
    view_dir: Vec3,
) -> Option<MaterialKindRt> {
    match node {
        SdfNode::Sphere {
            object_id,
            material_id,
            ..
        }
        | SdfNode::Box {
            object_id,
            material_id,
            ..
        }
        | SdfNode::Cylinder {
            object_id,
            material_id,
            ..
        }
        | SdfNode::Torus {
            object_id,
            material_id,
            ..
        }
        | SdfNode::ExtrudePolygon {
            object_id,
            material_id,
            ..
        }
        | SdfNode::Custom {
            object_id,
            material_id,
            ..
        } => {
            let leaf_hit = RayHit {
                object_id: *object_id,
                material_id: *material_id,
                ..hit
            };
            Some(resolve_leaf_material(
                material_for_id(&setup.materials, *material_id),
                setup,
                leaf_hit,
                view_dir,
            ))
        }
        SdfNode::DomainModifier { base, .. } | SdfNode::DistancePostModifier { base, .. } => {
            resolve_material_from_node(base, setup, hit, view_dir)
        }
        SdfNode::Union { lhs, rhs } => {
            let l = sdf_distance_info(lhs, hit.position);
            let r = sdf_distance_info(rhs, hit.position);
            if l.distance <= r.distance {
                resolve_material_from_node(lhs, setup, hit, view_dir)
            } else {
                resolve_material_from_node(rhs, setup, hit, view_dir)
            }
        }
        SdfNode::Intersect { lhs, rhs } => {
            let l = sdf_distance_info(lhs, hit.position);
            let r = sdf_distance_info(rhs, hit.position);
            if l.distance >= r.distance {
                resolve_material_from_node(lhs, setup, hit, view_dir)
            } else {
                resolve_material_from_node(rhs, setup, hit, view_dir)
            }
        }
        SdfNode::Subtract { lhs, rhs } => {
            let l = sdf_distance_info(lhs, hit.position);
            let r = sdf_distance_info(rhs, hit.position);
            if l.distance >= -r.distance {
                resolve_material_from_node(lhs, setup, hit, view_dir)
            } else {
                resolve_material_from_node(rhs, setup, hit, view_dir)
            }
        }
        SdfNode::UnionRound { lhs, rhs, r }
        | SdfNode::UnionChamfer { lhs, rhs, r }
        | SdfNode::UnionSoft { lhs, rhs, r }
        | SdfNode::IntersectRound { lhs, rhs, r }
        | SdfNode::IntersectChamfer { lhs, rhs, r } => {
            blend_node_materials(lhs, rhs, setup, hit, view_dir, *r, false)
        }
        SdfNode::UnionColumns { lhs, rhs, r, .. }
        | SdfNode::UnionStairs { lhs, rhs, r, .. }
        | SdfNode::IntersectColumns { lhs, rhs, r, .. }
        | SdfNode::IntersectStairs { lhs, rhs, r, .. } => {
            blend_node_materials(lhs, rhs, setup, hit, view_dir, *r, false)
        }
        SdfNode::DiffRound { lhs, rhs, r }
        | SdfNode::DiffChamfer { lhs, rhs, r }
        | SdfNode::Pipe { lhs, rhs, r }
        | SdfNode::Engrave { lhs, rhs, r } => {
            blend_node_materials(lhs, rhs, setup, hit, view_dir, *r, true)
        }
        SdfNode::DiffColumns { lhs, rhs, r, .. } | SdfNode::DiffStairs { lhs, rhs, r, .. } => {
            blend_node_materials(lhs, rhs, setup, hit, view_dir, *r, true)
        }
        SdfNode::Groove { lhs, rhs, ra, rb } | SdfNode::Tongue { lhs, rhs, ra, rb } => {
            blend_node_materials(lhs, rhs, setup, hit, view_dir, ra.max(*rb), true)
        }
        SdfNode::Slice { base, .. } => resolve_material_from_node(base, setup, hit, view_dir),
        SdfNode::Smooth { base, .. } => resolve_material_from_node(base, setup, hit, view_dir),
    }
}

fn resolve_split_material_from_node(
    node: &SdfNode,
    setup: &RenderSetup,
    hit: RayHit,
    view_dir: Vec3,
) -> Option<(MaterialKindRt, MaterialKindRt, f32)> {
    match node {
        SdfNode::Union { lhs, rhs } => {
            let l = sdf_distance_info(lhs, hit.position);
            let r = sdf_distance_info(rhs, hit.position);
            if l.distance <= r.distance {
                resolve_split_material_from_node(lhs, setup, hit, view_dir)
            } else {
                resolve_split_material_from_node(rhs, setup, hit, view_dir)
            }
        }
        SdfNode::Intersect { lhs, rhs } => {
            let l = sdf_distance_info(lhs, hit.position);
            let r = sdf_distance_info(rhs, hit.position);
            if l.distance >= r.distance {
                resolve_split_material_from_node(lhs, setup, hit, view_dir)
            } else {
                resolve_split_material_from_node(rhs, setup, hit, view_dir)
            }
        }
        SdfNode::Subtract { lhs, rhs } => {
            let l = sdf_distance_info(lhs, hit.position);
            let r = sdf_distance_info(rhs, hit.position);
            if l.distance >= -r.distance {
                resolve_split_material_from_node(lhs, setup, hit, view_dir)
            } else {
                resolve_split_material_from_node(rhs, setup, hit, view_dir)
            }
        }
        SdfNode::UnionRound { lhs, rhs, r }
        | SdfNode::UnionChamfer { lhs, rhs, r }
        | SdfNode::UnionSoft { lhs, rhs, r }
        | SdfNode::IntersectRound { lhs, rhs, r }
        | SdfNode::IntersectChamfer { lhs, rhs, r } => {
            split_node_materials(lhs, rhs, setup, hit, view_dir, *r, false)
        }
        SdfNode::UnionColumns { lhs, rhs, r, .. }
        | SdfNode::UnionStairs { lhs, rhs, r, .. }
        | SdfNode::IntersectColumns { lhs, rhs, r, .. }
        | SdfNode::IntersectStairs { lhs, rhs, r, .. } => {
            split_node_materials(lhs, rhs, setup, hit, view_dir, *r, false)
        }
        SdfNode::DiffRound { lhs, rhs, r }
        | SdfNode::DiffChamfer { lhs, rhs, r }
        | SdfNode::Pipe { lhs, rhs, r }
        | SdfNode::Engrave { lhs, rhs, r } => {
            split_node_materials(lhs, rhs, setup, hit, view_dir, *r, true)
        }
        SdfNode::DiffColumns { lhs, rhs, r, .. } | SdfNode::DiffStairs { lhs, rhs, r, .. } => {
            split_node_materials(lhs, rhs, setup, hit, view_dir, *r, true)
        }
        SdfNode::Groove { lhs, rhs, ra, rb } | SdfNode::Tongue { lhs, rhs, ra, rb } => {
            split_node_materials(lhs, rhs, setup, hit, view_dir, ra.max(*rb), true)
        }
        SdfNode::Smooth { base, .. } => {
            resolve_split_material_from_node(base, setup, hit, view_dir)
        }
        _ => None,
    }
}

fn blend_node_materials(
    lhs: &SdfNode,
    rhs: &SdfNode,
    setup: &RenderSetup,
    hit: RayHit,
    view_dir: Vec3,
    radius: f32,
    difference_mode: bool,
) -> Option<MaterialKindRt> {
    let l = sdf_distance_info(lhs, hit.position);
    let r = sdf_distance_info(rhs, hit.position);
    let left = resolve_material_from_node(lhs, setup, hit, view_dir)?;
    let right = resolve_material_from_node(rhs, setup, hit, view_dir)?;
    let k = radius.abs().max(1.0e-6);
    let t = if difference_mode {
        smoothstepf(0.0, 1.0, 0.5 + 0.5 * (((-r.distance) - l.distance) / k))
    } else {
        smoothstepf(0.0, 1.0, 0.5 + 0.5 * ((l.distance - r.distance) / k))
    };
    Some(blend_materials(left, right, t))
}

fn split_node_materials(
    lhs: &SdfNode,
    rhs: &SdfNode,
    setup: &RenderSetup,
    hit: RayHit,
    view_dir: Vec3,
    radius: f32,
    difference_mode: bool,
) -> Option<(MaterialKindRt, MaterialKindRt, f32)> {
    let l = sdf_distance_info(lhs, hit.position);
    let r = sdf_distance_info(rhs, hit.position);
    let left = resolve_material_from_node(lhs, setup, hit, view_dir)?;
    let right = resolve_material_from_node(rhs, setup, hit, view_dir)?;
    let k = radius.abs().max(1.0e-6);
    let t = if difference_mode {
        smoothstepf(0.0, 1.0, 0.5 + 0.5 * (((-r.distance) - l.distance) / k))
    } else {
        smoothstepf(0.0, 1.0, 0.5 + 0.5 * ((l.distance - r.distance) / k))
    };
    Some((left, right, t))
}

fn resolve_leaf_material(
    material: MaterialKindRt,
    setup: &RenderSetup,
    hit: RayHit,
    view_dir: Vec3,
) -> MaterialKindRt {
    let transform = setup
        .object_transforms
        .get(hit.object_id as usize)
        .copied()
        .unwrap_or_else(PrimitiveTransform::identity);
    let local_position = to_local(hit.position, transform);
    let runtime_color = resolve_dynamic_color(
        &setup.state,
        &setup.material_def_names,
        &setup.dynamic_material_overrides,
        material,
        hit,
        local_position,
        view_dir,
    );
    let runtime_roughness = resolve_dynamic_number(
        &setup.state,
        &setup.material_def_names,
        &setup.dynamic_material_overrides,
        material,
        hit,
        local_position,
        view_dir,
        "roughness",
    );
    let runtime_ior = resolve_dynamic_number(
        &setup.state,
        &setup.material_def_names,
        &setup.dynamic_material_overrides,
        material,
        hit,
        local_position,
        view_dir,
        "ior",
    );
    let runtime_thin_walled = resolve_dynamic_number(
        &setup.state,
        &setup.material_def_names,
        &setup.dynamic_material_overrides,
        material,
        hit,
        local_position,
        view_dir,
        "thin_walled",
    );
    let runtime_medium = resolve_dynamic_object(
        &setup.state,
        &setup.material_def_names,
        &setup.dynamic_material_overrides,
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
        &setup.dynamic_material_overrides,
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
        &setup.dynamic_material_overrides,
        material,
        hit,
        local_position,
        view_dir,
        "emission_color",
    );
    let runtime_emission_strength = resolve_dynamic_number(
        &setup.state,
        &setup.material_def_names,
        &setup.dynamic_material_overrides,
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
        MaterialKindRt::Blend(blend) => MaterialKindRt::Blend(blend),
    }
}

fn smoothstepf(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0).max(1.0e-6)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn blend_medium(a: Option<MediumParams>, b: Option<MediumParams>, t: f32) -> Option<MediumParams> {
    match (a, b) {
        (Some(a), Some(b)) => Some(MediumParams {
            ior: a.ior + (b.ior - a.ior) * t,
            absorption_color: lerp_spectrum(a.absorption_color, b.absorption_color, t),
            density: a.density + (b.density - a.density) * t,
        }),
        (Some(a), None) => {
            if t < 0.5 {
                Some(a)
            } else {
                None
            }
        }
        (None, Some(b)) => {
            if t >= 0.5 {
                Some(b)
            } else {
                None
            }
        }
        (None, None) => None,
    }
}

fn blend_subsurface(
    a: Option<SubsurfaceParams>,
    b: Option<SubsurfaceParams>,
    t: f32,
) -> Option<SubsurfaceParams> {
    match (a, b) {
        (Some(a), Some(b)) => Some(SubsurfaceParams {
            color: lerp_spectrum(a.color, b.color, t),
            radius: ApiVec3::new(
                a.radius.x + (b.radius.x - a.radius.x) * t,
                a.radius.y + (b.radius.y - a.radius.y) * t,
                a.radius.z + (b.radius.z - a.radius.z) * t,
            ),
            anisotropy: a.anisotropy + (b.anisotropy - a.anisotropy) * t,
            scale: a.scale + (b.scale - a.scale) * t,
        }),
        (Some(a), None) => {
            if t < 0.5 {
                Some(a)
            } else {
                None
            }
        }
        (None, Some(b)) => {
            if t >= 0.5 {
                Some(b)
            } else {
                None
            }
        }
        (None, None) => None,
    }
}

fn blend_params(mut a: MaterialParams, b: MaterialParams, t: f32) -> MaterialParams {
    a.color = lerp_spectrum(a.color, b.color, t);
    a.roughness = a.roughness + (b.roughness - a.roughness) * t;
    a.ior = a.ior + (b.ior - a.ior) * t;
    a.thin_walled = if t < 0.5 {
        a.thin_walled
    } else {
        b.thin_walled
    };
    a.emission_color = lerp_spectrum(a.emission_color, b.emission_color, t);
    a.emission_strength = a.emission_strength + (b.emission_strength - a.emission_strength) * t;
    a.medium = blend_medium(a.medium, b.medium, t);
    a.subsurface = blend_subsurface(a.subsurface, b.subsurface, t);
    a.pattern = None;
    a.dynamic_material_id = None;
    a.dynamic_override_id = None;
    a
}

fn blend_materials(a: MaterialKindRt, b: MaterialKindRt, t: f32) -> MaterialKindRt {
    match (a, b) {
        (MaterialKindRt::Lambert(a), MaterialKindRt::Lambert(b)) => {
            MaterialKindRt::Lambert(blend_params(a, b, t))
        }
        (MaterialKindRt::Metal(a), MaterialKindRt::Metal(b)) => {
            MaterialKindRt::Metal(blend_params(a, b, t))
        }
        (MaterialKindRt::Dielectric(a), MaterialKindRt::Dielectric(b)) => {
            MaterialKindRt::Dielectric(blend_params(a, b, t))
        }
        (a, b) => MaterialKindRt::Blend(BlendedMaterial {
            a_model: dominant_material_model(a),
            a_params: dominant_material_params(a),
            b_model: dominant_material_model(b),
            b_params: dominant_material_params(b),
            t: t.clamp(0.0, 1.0),
        }),
    }
}

fn dominant_material_model(material: MaterialKindRt) -> MaterialKindTag {
    match material {
        MaterialKindRt::Lambert(_) => MaterialKindTag::Lambert,
        MaterialKindRt::Metal(_) => MaterialKindTag::Metal,
        MaterialKindRt::Dielectric(_) => MaterialKindTag::Dielectric,
        MaterialKindRt::Blend(blend) => {
            if blend.t < 0.5 {
                blend.a_model
            } else {
                blend.b_model
            }
        }
    }
}

fn dominant_material_params(material: MaterialKindRt) -> MaterialParams {
    match material {
        MaterialKindRt::Lambert(params)
        | MaterialKindRt::Metal(params)
        | MaterialKindRt::Dielectric(params) => params,
        MaterialKindRt::Blend(blend) => {
            if blend.t < 0.5 {
                blend.a_params
            } else {
                blend.b_params
            }
        }
    }
}

fn resolve_dynamic_color(
    state: &EvalState,
    material_def_names: &[String],
    dynamic_material_overrides: &[ObjectValue],
    material: MaterialKindRt,
    hit: RayHit,
    local_position: Vec3,
    view_dir: Vec3,
) -> Option<Spectrum> {
    let dynamic_material_id = dominant_material_params(material).dynamic_material_id?;
    let dynamic_override = dynamic_material_override(material, dynamic_material_overrides);
    let name = material_def_names.get(dynamic_material_id as usize)?;
    let ctx = make_shading_context(hit, local_position, view_dir);
    let value =
        eval_material_function_with_overrides(state, name, "color", ctx, dynamic_override).ok()?;
    spectrum_from_value(&value)
}

#[allow(clippy::too_many_arguments)]
fn resolve_dynamic_spectrum(
    state: &EvalState,
    material_def_names: &[String],
    dynamic_material_overrides: &[ObjectValue],
    material: MaterialKindRt,
    hit: RayHit,
    local_position: Vec3,
    view_dir: Vec3,
    function_name: &str,
) -> Option<Spectrum> {
    let dynamic_material_id = dominant_material_params(material).dynamic_material_id?;
    let dynamic_override = dynamic_material_override(material, dynamic_material_overrides);
    let name = material_def_names.get(dynamic_material_id as usize)?;
    let ctx = make_shading_context(hit, local_position, view_dir);
    let value =
        eval_material_function_with_overrides(state, name, function_name, ctx, dynamic_override)
            .ok()?;
    spectrum_from_value(&value)
}

#[allow(clippy::too_many_arguments)]
fn resolve_dynamic_number(
    state: &EvalState,
    material_def_names: &[String],
    dynamic_material_overrides: &[ObjectValue],
    material: MaterialKindRt,
    hit: RayHit,
    local_position: Vec3,
    view_dir: Vec3,
    function_name: &str,
) -> Option<f64> {
    let dynamic_material_id = dominant_material_params(material).dynamic_material_id?;
    let dynamic_override = dynamic_material_override(material, dynamic_material_overrides);
    let name = material_def_names.get(dynamic_material_id as usize)?;
    let ctx = make_shading_context(hit, local_position, view_dir);
    let value =
        eval_material_function_with_overrides(state, name, function_name, ctx, dynamic_override)
            .ok()?;
    let Value::Number(v) = value else {
        return None;
    };
    Some(v)
}

#[allow(clippy::too_many_arguments)]
fn resolve_dynamic_normal(
    state: &EvalState,
    material_def_names: &[String],
    dynamic_material_overrides: &[ObjectValue],
    material: MaterialKindRt,
    hit: RayHit,
    local_position: Vec3,
    view_dir: Vec3,
    geometric_normal: Vec3,
) -> Option<Vec3> {
    let dynamic_material_id = dominant_material_params(material).dynamic_material_id?;
    let dynamic_override = dynamic_material_override(material, dynamic_material_overrides);
    let name = material_def_names.get(dynamic_material_id as usize)?;
    let ctx = make_shading_context_with_normal(hit, local_position, view_dir, geometric_normal);
    let value =
        eval_material_function_with_overrides(state, name, "normal", ctx, dynamic_override).ok()?;
    let mut normal = vec3_from_value(&value)?.normalize();
    if normal.length() <= 1.0e-6 {
        return None;
    }
    if normal.dot(geometric_normal) < 0.0 {
        normal = normal.mul(-1.0);
    }
    Some(normal)
}

#[allow(clippy::too_many_arguments)]
fn resolve_dynamic_object(
    state: &EvalState,
    material_def_names: &[String],
    dynamic_material_overrides: &[ObjectValue],
    material: MaterialKindRt,
    hit: RayHit,
    local_position: Vec3,
    view_dir: Vec3,
    function_name: &str,
) -> Option<Value> {
    let dynamic_material_id = dominant_material_params(material).dynamic_material_id?;
    let dynamic_override = dynamic_material_override(material, dynamic_material_overrides);
    let name = material_def_names.get(dynamic_material_id as usize)?;
    let ctx = make_shading_context(hit, local_position, view_dir);
    eval_material_function_with_overrides(state, name, function_name, ctx, dynamic_override).ok()
}

fn dynamic_material_override(
    material: MaterialKindRt,
    dynamic_material_overrides: &[ObjectValue],
) -> Option<&ObjectValue> {
    let override_id = dominant_material_params(material).dynamic_override_id?;
    dynamic_material_overrides.get(override_id as usize)
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
        Value::String(_) | Value::Array(_) | Value::Function(_) => None,
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
        AccelMode, RaySettings, RenderOptions, extract_scene_render_settings,
        render_depth_png_with_accel, render_ray_progressive_with_accel,
    };

    fn empty_state(bindings: HashMap<String, Binding>) -> EvalState {
        EvalState {
            bindings,
            function_defs: HashMap::new(),
            compiled_functions: HashMap::new(),
            jitted_functions: HashMap::new(),
            compiled_material_functions: HashMap::new(),
            jitted_material_functions: HashMap::new(),
            jitted_material_vec3_functions: HashMap::new(),
            jitted_sdf_distance_functions: HashMap::new(),
            jitted_sdf_vec3_functions: HashMap::new(),
            jitted_sdf_functions: HashMap::new(),
            compiled_sdf_functions: HashMap::new(),
            material_defs: HashMap::new(),
            sdf_defs: HashMap::new(),
            environment_defs: HashMap::new(),
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
        point_light_fields.insert("color".to_string(), vec3_value(1.0, 0.5, 0.25));
        point_light_fields.insert("intensity".to_string(), Value::Number(6.0));
        let point_light = Value::Object(crate::ObjectValue {
            type_name: Some("PointLight".to_string()),
            fields: point_light_fields,
        });

        let mut env_fields = HashMap::new();
        env_fields.insert("color".to_string(), vec3_value(0.5, 0.5, 1.0));
        env_fields.insert("intensity".to_string(), Value::Number(0.08));
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
    fn traces_with_sphere_light() {
        let mut sphere_fields = HashMap::new();
        sphere_fields.insert("radius".to_string(), Value::Number(0.9));
        let sphere = Value::Object(crate::ObjectValue {
            type_name: Some("Sphere".to_string()),
            fields: sphere_fields,
        });

        let mut camera_fields = HashMap::new();
        camera_fields.insert("origin".to_string(), vec3_value(0.0, 0.8, 5.0));
        camera_fields.insert("target".to_string(), vec3_value(0.0, 0.2, 0.0));
        camera_fields.insert("fov_y".to_string(), Value::Number(35.0));
        let camera = Value::Object(crate::ObjectValue {
            type_name: Some("Camera".to_string()),
            fields: camera_fields,
        });

        let mut light_fields = HashMap::new();
        light_fields.insert("position".to_string(), vec3_value(2.0, 3.0, 3.5));
        light_fields.insert("radius".to_string(), Value::Number(0.7));
        light_fields.insert("color".to_string(), vec3_value(1.0, 0.94, 0.86));
        light_fields.insert("intensity".to_string(), Value::Number(40.0));
        light_fields.insert("samples".to_string(), Value::Number(4.0));
        let sphere_light = Value::Object(crate::ObjectValue {
            type_name: Some("SphereLight".to_string()),
            fields: light_fields,
        });

        let mut bindings = HashMap::new();
        bindings.insert(
            "scene".to_string(),
            Binding {
                mutable: false,
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
            "key".to_string(),
            Binding {
                mutable: false,
                value: sphere_light,
            },
        );

        let image = render_ray_progressive_with_accel(
            &empty_state(bindings),
            RenderOptions {
                width: 48,
                height: 48,
                max_steps: 320,
                max_dist: 30.0,
                epsilon: 0.0002,
                step_scale: 0.7,
                camera_z: 6.0,
                fov_y_degrees: 35.0,
            },
            AccelMode::Naive,
            RaySettings {
                max_depth: 2,
                tile_size: 48,
                aa_samples: 1,
                debug_aov: None,
            },
            |_, _| Ok(()),
        )
        .expect("trace with sphere light should succeed");

        let has_non_black = image.pixels().any(|pixel| pixel.0 != [0, 0, 0]);
        assert!(
            has_non_black,
            "sphere light trace should produce visible output"
        );
    }

    #[test]
    fn extracts_render_settings_from_bindings() {
        let mut render_fields = HashMap::new();
        render_fields.insert("width".to_string(), Value::Number(640.0));
        render_fields.insert("height".to_string(), Value::Number(360.0));
        render_fields.insert("max_steps".to_string(), Value::Number(180.0));
        render_fields.insert("max_dist".to_string(), Value::Number(60.0));
        render_fields.insert("epsilon".to_string(), Value::Number(0.0005));
        render_fields.insert("step_scale".to_string(), Value::Number(0.7));
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
        assert_eq!(settings.step_scale, Some(0.7));
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
    fn resolves_dynamic_material_instance_overrides() {
        let source = r#"
            material CheckerFloor {
              model: Lambert;
              roughness = 1.0;
              let color_a = #f1f3f6;
              let color_b = #6f7784;
              let scale = 3.4;

              fn checker(p, scale) {
                let sx = step(0.0, sin(p.x * scale));
                let sz = step(0.0, sin(p.z * scale));
                return abs(sx - sz);
              }

              fn color(ctx) {
                let m = checker(ctx.local_position, scale);
                return mix(color_a, color_b, m);
              }
            };

            let scene = Box {
              size: vec3(4.0, 0.5, 4.0),
              material: CheckerFloor {
                color_a: #ff0000,
                color_b: #0000ff,
                scale: 3.1415926535
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
        let setup = super::build_render_setup(&state, &scene, RenderOptions::default());

        let hit_a = super::RayHit {
            t: 1.0,
            position: super::Vec3::new(1.0, 0.25, 0.0),
            normal: super::Vec3::new(0.0, 1.0, 0.0),
            front_face: true,
            object_id: 1,
            material_id: 1,
        };
        let hit_b = super::RayHit {
            t: 1.0,
            position: super::Vec3::new(1.5, 0.25, 0.0),
            normal: super::Vec3::new(0.0, 1.0, 0.0),
            front_face: true,
            object_id: 1,
            material_id: 1,
        };

        let super::MaterialKindRt::Lambert(mat_a) =
            super::resolve_material_at_hit(&setup, hit_a, super::Vec3::new(0.0, 1.0, 1.0))
        else {
            panic!("expected Lambert material");
        };
        let super::MaterialKindRt::Lambert(mat_b) =
            super::resolve_material_at_hit(&setup, hit_b, super::Vec3::new(0.0, 1.0, 1.0))
        else {
            panic!("expected Lambert material");
        };

        assert!(mat_a.color.r > 0.9);
        assert!(mat_a.color.b < 0.1);
        assert!(mat_b.color.b > 0.9);
        assert!(mat_b.color.r < 0.1);
    }

    #[test]
    fn renders_cylinder_torus_and_extruded_polygon_with_rotation() {
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

        let mut poly_fields = HashMap::new();
        poly_fields.insert("sides".to_string(), Value::Number(6.0));
        poly_fields.insert("radius".to_string(), Value::Number(0.75));
        poly_fields.insert("height".to_string(), Value::Number(0.6));
        poly_fields.insert("x".to_string(), Value::Number(-1.2));
        poly_fields.insert("y".to_string(), Value::Number(-0.2));
        let polygon = Value::Object(crate::ObjectValue {
            type_name: Some("ExtrudePolygon".to_string()),
            fields: poly_fields,
        });

        let mut add_fields = HashMap::new();
        add_fields.insert("lhs".to_string(), cylinder);
        add_fields.insert("rhs".to_string(), torus);
        let combined = Value::Object(crate::ObjectValue {
            type_name: Some("add".to_string()),
            fields: add_fields,
        });

        let mut add_fields = HashMap::new();
        add_fields.insert("lhs".to_string(), combined);
        add_fields.insert("rhs".to_string(), polygon);
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
    fn resolves_environment_color_on_miss() {
        let source = r#"
            environment Sky {
              let zenith = #4d74c7;
              let horizon = #d8e7ff;

              fn color(dir) {
                let t = clamp(dir.y * 0.5 + 0.5, 0.0, 1.0);
                return mix(horizon, zenith, t);
              }
            };

            let scene = Sphere {
              radius: 1.0
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
        let sky = super::environment_color(&setup, super::Vec3::new(0.0, 1.0, 0.0))
            .expect("environment color should resolve");
        assert!(sky.b > sky.r);
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
