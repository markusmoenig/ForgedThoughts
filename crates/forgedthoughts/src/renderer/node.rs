use std::sync::mpsc;
use std::time::Instant;
use std::env;

use image::{ImageError, RgbImage};
use rayon::prelude::*;

use crate::{
    EvalState, GraphRenderSourceKind, compile_specialized_height_node_eval_function,
};

use super::{RenderError, RenderProgress};
use super::height_source::HeightSampler;
use super::wgpu_field::try_render_field_wgpu;

#[derive(Debug, Clone, Copy)]
pub struct NodeRenderSettings {
    pub width: u32,
    pub height: u32,
    pub tile_size: u32,
    /// World-space coordinate range: pixels map to [0, world_size] in both X and Z.
    pub world_size: f32,
}

impl Default for NodeRenderSettings {
    fn default() -> Self {
        Self {
            width: 512,
            height: 512,
            tile_size: 64,
            world_size: 1.0,
        }
    }
}

/// Render a node graph to a grayscale PNG image.
///
/// Looks for a top-level binding named `graph` in `state`, determines its node
/// type, and evaluates the graph for every pixel.
///
/// Height nodes are evaluated through `eval(ctx)`.
///
/// The renderer first tries a specialized `eval(ctx)` fast path that lowers a
/// fixed `HeightContext` into a scalar kernel, then falls back to interpreted
/// `eval(ctx)` if specialization is not available.
/// The returned float is clamped to [0, 1] and written as an 8-bit grayscale
/// value (stored as RGB).
pub fn render_node_png<F>(
    state: &EvalState,
    source_kind: GraphRenderSourceKind,
    settings: NodeRenderSettings,
    mut progress_cb: F,
) -> Result<RgbImage, RenderError>
where
    F: FnMut(RenderProgress, &RgbImage) -> Result<(), ImageError> + Send,
{
    let require_gpu_field = matches!(
        env::var("FORGEDTHOUGHTS_REQUIRE_GPU_FIELD").ok().as_deref(),
        Some("1" | "true" | "TRUE" | "yes" | "YES" | "on" | "ON")
    );

    let graph_binding = state
        .bindings
        .get("graph")
        .ok_or(RenderError::MissingGraph)?;

    let graph_obj = match &graph_binding.value {
        crate::Value::Object(obj) => obj.clone(),
        _ => return Err(RenderError::MissingGraph),
    };

    let gpu_start = Instant::now();
    if matches!(source_kind, GraphRenderSourceKind::FieldScalar) {
        match try_render_field_wgpu(state, source_kind, &graph_obj, settings) {
            Ok(Some(image)) => {
                progress_cb(
                    RenderProgress {
                        tiles_done: 1,
                        tiles_total: 1,
                        elapsed_ms: gpu_start.elapsed().as_millis(),
                    },
                    &image,
                )?;
                return Ok(image);
            }
            Ok(None) => {
                if require_gpu_field {
                    return Err(RenderError::Backend(
                        "GPU field backend was required but this graph could not be translated to GPU".to_string(),
                    ));
                }
            }
            Err(err) => {
                if require_gpu_field {
                    return Err(err);
                }
            }
        }
    }

    let sampler = HeightSampler::new(state, graph_obj.clone(), source_kind);
    let node_name = graph_obj
        .type_name
        .clone()
        .ok_or(RenderError::MissingGraph)?;
    let compiled_eval = matches!(source_kind, GraphRenderSourceKind::PointScalar)
        .then(|| compile_specialized_height_node_eval_function(state, &node_name, Some(&graph_obj)))
        .flatten();

    let width = settings.width.max(1);
    let height = settings.height.max(1);
    let tile_size = settings.tile_size.max(8);
    let world_size = settings.world_size.max(f32::EPSILON);

    let tiles_x = width.div_ceil(tile_size);
    let tiles_y = height.div_ceil(tile_size);
    let tiles_total = tiles_x * tiles_y;

    let mut image = RgbImage::new(width, height);

    let tile_indices: Vec<(u32, u32)> = (0..tiles_y)
        .flat_map(|ty| (0..tiles_x).map(move |tx| (tx, ty)))
        .collect();

    let (sender, receiver) = mpsc::channel::<(u32, u32, Vec<u8>)>();

    // EvalState: Sync — safe to share across rayon threads.
    tile_indices.par_iter().for_each_with(sender, |s, &(tile_x, tile_y)| {
        let sampler = sampler.clone();
        let px_start = tile_x * tile_size;
        let py_start = tile_y * tile_size;
        let px_end = (px_start + tile_size).min(width);
        let py_end = (py_start + tile_size).min(height);

        let tile_w = (px_end - px_start) as usize;
        let tile_h = (py_end - py_start) as usize;
        let mut pixels = vec![0u8; tile_w * tile_h * 3];

        for (iy, py) in (py_start..py_end).enumerate() {
            for (ix, px) in (px_start..px_end).enumerate() {
                let x = (px as f32 / width as f32) * world_size;
                let z = (py as f32 / height as f32) * world_size;

                let sample = compiled_eval
                    .as_ref()
                    .and_then(|jit| jit.invoke(&[x, z]))
                    .or_else(|| sampler.sample_root_scalar(x, z))
                    .unwrap_or(0.0);

                let byte = (sample.clamp(0.0, 1.0) * 255.0).round() as u8;
                let idx = (iy * tile_w + ix) * 3;
                pixels[idx] = byte;
                pixels[idx + 1] = byte;
                pixels[idx + 2] = byte;
            }
        }

        let _ = s.send((px_start, py_start, pixels));
    });

    let start = Instant::now();
    let mut tiles_done = 0u32;
    for (px_start, py_start, pixels) in receiver {
        let px_end = (px_start + tile_size).min(width);
        let py_end = (py_start + tile_size).min(height);
        let tile_w = (px_end - px_start) as usize;

        for (iy, py) in (py_start..py_end).enumerate() {
            for (ix, px) in (px_start..px_end).enumerate() {
                let idx = (iy * tile_w + ix) * 3;
                image.put_pixel(px, py, image::Rgb([pixels[idx], pixels[idx + 1], pixels[idx + 2]]));
            }
        }

        tiles_done += 1;
        let elapsed_ms = start.elapsed().as_millis();
        if let Err(err) = progress_cb(
            RenderProgress { tiles_done, tiles_total, elapsed_ms },
            &image,
        ) {
            return Err(RenderError::Image(err));
        }
    }

    Ok(image)
}
