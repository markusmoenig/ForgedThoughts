use std::collections::{HashMap, HashSet};
use std::sync::mpsc;

use bytemuck::{Pod, Zeroable};
use image::RgbImage;

use crate::ast::{BinaryOp, Expr, MaterialFunctionStatement, MaterialStatement, UnaryOp};
use crate::{EvalState, GraphRenderSourceKind, ObjectValue, Value};

use super::RenderError;
use super::height_source::HeightSampler;
use super::node::NodeRenderSettings;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct FieldParams {
    width: u32,
    height: u32,
    world_size: f32,
    _pad: f32,
}

#[derive(Clone)]
struct GpuInput {
    binding_name: String,
    object: ObjectValue,
}

#[derive(Clone)]
pub struct GpuFieldRaster {
    pub width: u32,
    pub height: u32,
    pub world_size: f32,
    pub values: Vec<f32>,
}

impl GpuFieldRaster {
    pub fn sample_bilinear(&self, x: f32, z: f32) -> f32 {
        if self.width == 0 || self.height == 0 {
            return 0.0;
        }
        let px = (x / self.world_size) * self.width as f32;
        let pz = (z / self.world_size) * self.height as f32;
        let max_x = (self.width.saturating_sub(1)) as f32;
        let max_z = (self.height.saturating_sub(1)) as f32;
        let sx = px.clamp(0.0, max_x);
        let sz = pz.clamp(0.0, max_z);

        let x0 = sx.floor() as u32;
        let z0 = sz.floor() as u32;
        let x1 = (x0 + 1).min(self.width.saturating_sub(1));
        let z1 = (z0 + 1).min(self.height.saturating_sub(1));
        let tx = sx - x0 as f32;
        let tz = sz - z0 as f32;

        let idx = |xx: u32, zz: u32| -> usize { (zz * self.width + xx) as usize };
        let v00 = *self.values.get(idx(x0, z0)).unwrap_or(&0.0);
        let v10 = *self.values.get(idx(x1, z0)).unwrap_or(&0.0);
        let v01 = *self.values.get(idx(x0, z1)).unwrap_or(&0.0);
        let v11 = *self.values.get(idx(x1, z1)).unwrap_or(&0.0);

        let vx0 = v00 + (v10 - v00) * tx;
        let vx1 = v01 + (v11 - v01) * tx;
        (vx0 + (vx1 - vx0) * tz).clamp(0.0, 1.0)
    }
}

pub fn try_render_field_wgpu(
    state: &EvalState,
    source_kind: GraphRenderSourceKind,
    graph_obj: &ObjectValue,
    settings: NodeRenderSettings,
) -> Result<Option<RgbImage>, RenderError> {
    let Some(raster) = try_rasterize_field_wgpu(state, source_kind, graph_obj, settings)? else {
        return Ok(None);
    };

    let width = raster.width;
    let height = raster.height;
    let mut image = RgbImage::new(width, height);
    for py in 0..height {
        for px in 0..width {
            let v = raster.values[(py * width + px) as usize].clamp(0.0, 1.0);
            let byte = (v * 255.0).round() as u8;
            image.put_pixel(px, py, image::Rgb([byte, byte, byte]));
        }
    }
    Ok(Some(image))
}

pub fn try_rasterize_field_wgpu(
    state: &EvalState,
    source_kind: GraphRenderSourceKind,
    graph_obj: &ObjectValue,
    settings: NodeRenderSettings,
) -> Result<Option<GpuFieldRaster>, RenderError> {
    if !matches!(source_kind, GraphRenderSourceKind::FieldScalar) {
        return Ok(None);
    }

    let Some(node_name) = graph_obj.type_name.as_deref() else {
        return Ok(None);
    };
    let Some(node_def) = state.node_defs.get(node_name) else {
        return Ok(None);
    };

    let (numeric_bindings, object_bindings) = resolve_node_bindings(node_def, graph_obj)?;
    let eval_fn = node_def
        .statements
        .iter()
        .find_map(|stmt| match stmt {
            MaterialStatement::Function { name, params, body } if name == "eval" && params.len() == 1 => {
                Some((params[0].clone(), body.clone()))
            }
            _ => None,
        })
        .ok_or_else(|| RenderError::Backend(format!("GPU translation failed: missing eval(ctx) for node '{node_name}'")))?;

    let mut sample_inputs = HashSet::new();
    for stmt in &eval_fn.1 {
        collect_sample_input_names_stmt(stmt, &mut sample_inputs);
    }
    for stmt in &node_def.statements {
        if let MaterialStatement::Function { body, .. } = stmt {
            for line in body {
                collect_sample_input_names_stmt(line, &mut sample_inputs);
            }
        }
    }

    let mut gpu_inputs = Vec::new();
    for input_name in sample_inputs {
        let Some(obj) = object_bindings.get(&input_name).cloned() else {
            return Err(RenderError::Backend(format!(
                "GPU translation failed: sample input '{input_name}' is not an object binding"
            )));
        };
        gpu_inputs.push(GpuInput {
            binding_name: input_name,
            object: obj,
        });
    }

    let shader = build_wgsl_shader(node_def, &numeric_bindings, &gpu_inputs, &eval_fn.0)?;
    let input_buffers = rasterize_input_buffers(state, graph_obj, &gpu_inputs, settings);
    let width = settings.width.max(1);
    let height = settings.height.max(1);
    let world_size = settings.world_size.max(f32::EPSILON);
    let values = render_field_compute(&shader, &input_buffers, width, height, world_size)?;
    Ok(Some(GpuFieldRaster {
        width,
        height,
        world_size,
        values,
    }))
}

fn resolve_node_bindings(
    def: &crate::NodeDef,
    overrides: &ObjectValue,
) -> Result<(HashMap<String, f32>, HashMap<String, ObjectValue>), RenderError> {
    let mut nums = HashMap::new();
    let mut objs = HashMap::new();

    for stmt in &def.statements {
        let MaterialStatement::Binding { name, expr } = stmt else {
            continue;
        };
        if let Some(override_value) = overrides.fields.get(name) {
            match override_value {
                Value::Number(n) => {
                    nums.insert(name.clone(), *n);
                    continue;
                }
                Value::Object(o) => {
                    objs.insert(name.clone(), o.clone());
                    continue;
                }
                _ => {
                    return Err(RenderError::Backend(format!(
                        "GPU translation failed: unsupported override type for '{name}'"
                    )));
                }
            }
        }

        if let Some(n) = eval_const_number(expr, &nums) {
            nums.insert(name.clone(), n);
            continue;
        }
        if let Some(obj) = eval_const_object(expr, &nums) {
            objs.insert(name.clone(), obj);
            continue;
        }
    }

    Ok((nums, objs))
}

fn eval_const_number(expr: &Expr, nums: &HashMap<String, f32>) -> Option<f32> {
    match expr {
        Expr::Number(v) => Some(*v as f32),
        Expr::Ident(name) => nums.get(name).copied(),
        Expr::Unary { op: UnaryOp::Neg, expr } => Some(-eval_const_number(expr, nums)?),
        Expr::Binary { lhs, op, rhs } => {
            let l = eval_const_number(lhs, nums)?;
            let r = eval_const_number(rhs, nums)?;
            Some(match op {
                BinaryOp::Add => l + r,
                BinaryOp::Sub => l - r,
                BinaryOp::Mul => l * r,
                BinaryOp::Div => l / r,
                BinaryOp::Intersect => return None,
            })
        }
        _ => None,
    }
}

fn eval_const_object(expr: &Expr, nums: &HashMap<String, f32>) -> Option<ObjectValue> {
    let Expr::ObjectLiteral { type_name, fields } = expr else {
        return None;
    };
    let mut out = HashMap::new();
    for (name, value_expr) in fields {
        if let Some(n) = eval_const_number(value_expr, nums) {
            out.insert(name.clone(), Value::Number(n));
            continue;
        }
        if let Some(obj) = eval_const_object(value_expr, nums) {
            out.insert(name.clone(), Value::Object(obj));
            continue;
        }
        return None;
    }
    Some(ObjectValue {
        type_name: Some(type_name.clone()),
        fields: out,
    })
}

fn rasterize_input_buffers(
    state: &EvalState,
    root_graph_obj: &ObjectValue,
    inputs: &[GpuInput],
    settings: NodeRenderSettings,
) -> Vec<Vec<f32>> {
    let width = settings.width.max(1);
    let height = settings.height.max(1);
    let world_size = settings.world_size.max(f32::EPSILON);
    let sampler = HeightSampler::new(
        state,
        root_graph_obj.clone(),
        GraphRenderSourceKind::FieldScalar,
    );

    inputs
        .iter()
        .map(|input| {
            let mut data = vec![0.0f32; (width * height) as usize];
            for py in 0..height {
                for px in 0..width {
                    let x = (px as f32 / width as f32) * world_size;
                    let z = (py as f32 / height as f32) * world_size;
                    data[(py * width + px) as usize] =
                        sampler.sample_point_object(&input.object, x, z).unwrap_or(0.0);
                }
            }
            data
        })
        .collect()
}

fn build_wgsl_shader(
    def: &crate::NodeDef,
    nums: &HashMap<String, f32>,
    inputs: &[GpuInput],
    eval_param_name: &str,
) -> Result<String, RenderError> {
    let mut out = String::new();
    out.push_str(
        "struct Params { width: u32, height: u32, world_size: f32, _pad: f32 }\n\
         @group(0) @binding(",
    );
    out.push_str(&(inputs.len() + 1).to_string());
    out.push_str(") var<uniform> params: Params;\n");

    for (i, input) in inputs.iter().enumerate() {
        out.push_str(&format!(
            "@group(0) @binding({i}) var<storage, read> input_{}: array<f32>;\n",
            input.binding_name
        ));
    }
    out.push_str(&format!(
        "@group(0) @binding({}) var<storage, read_write> output_values: array<f32>;\n",
        inputs.len()
    ));

    for input in inputs {
        let buf = format!("input_{}", input.binding_name);
        out.push_str(&format!(
            "fn sample_{}(x: f32, z: f32) -> f32 {{\n",
            input.binding_name
        ));
        out.push_str("  let px = (x / params.world_size) * f32(params.width);\n");
        out.push_str("  let pz = (z / params.world_size) * f32(params.height);\n");
        out.push_str(&sample_bilinear_fn_body(&buf));
        out.push_str("}\n");
    }

    for stmt in &def.statements {
        if let MaterialStatement::Function { name, params, body } = stmt {
            let fn_name = if name == "eval" { "eval_field" } else { name.as_str() };
            let mut param_list = Vec::new();
            for p in params {
                if p == eval_param_name {
                    param_list.push(format!("{p}_x: f32"));
                    param_list.push(format!("{p}_z: f32"));
                } else {
                    param_list.push(format!("{p}: f32"));
                }
            }
            out.push_str(&format!("fn {fn_name}({}) -> f32 {{\n", param_list.join(", ")));
            for line in body {
                let s = translate_stmt(line, nums, eval_param_name)?;
                out.push_str(&s);
            }
            out.push_str("}\n");
        }
    }

    out.push_str(
        "@compute @workgroup_size(8, 8, 1)\n\
         fn main(@builtin(global_invocation_id) gid: vec3<u32>) {\n\
           if (gid.x >= params.width || gid.y >= params.height) { return; }\n\
           let idx = gid.y * params.width + gid.x;\n\
           let x = (f32(gid.x) / f32(params.width)) * params.world_size;\n\
           let z = (f32(gid.y) / f32(params.height)) * params.world_size;\n\
           output_values[idx] = clamp(eval_field(x, z), 0.0, 1.0);\n\
         }\n",
    );

    Ok(out)
}

fn sample_bilinear_fn_body(buf_name: &str) -> String {
    format!(
        "  let max_x = f32(params.width - 1u);\n\
         let max_z = f32(params.height - 1u);\n\
         let sx = clamp(px, 0.0, max_x);\n\
         let sz = clamp(pz, 0.0, max_z);\n\
         let x0 = u32(floor(sx));\n\
         let z0 = u32(floor(sz));\n\
         let x1 = min(x0 + 1u, params.width - 1u);\n\
         let z1 = min(z0 + 1u, params.height - 1u);\n\
         let tx = sx - f32(x0);\n\
         let tz = sz - f32(z0);\n\
         let i00 = z0 * params.width + x0;\n\
         let i10 = z0 * params.width + x1;\n\
         let i01 = z1 * params.width + x0;\n\
         let i11 = z1 * params.width + x1;\n\
         let v00 = {buf_name}[i00];\n\
         let v10 = {buf_name}[i10];\n\
         let v01 = {buf_name}[i01];\n\
         let v11 = {buf_name}[i11];\n\
         let vx0 = mix(v00, v10, tx);\n\
         let vx1 = mix(v01, v11, tx);\n\
         return mix(vx0, vx1, tz);\n"
    )
}

fn translate_stmt(
    stmt: &MaterialFunctionStatement,
    nums: &HashMap<String, f32>,
    ctx_name: &str,
) -> Result<String, RenderError> {
    match stmt {
        MaterialFunctionStatement::Binding { name, expr } => {
            let expr = translate_expr(expr, nums, ctx_name)?;
            Ok(format!("  var {name}: f32 = {expr};\n"))
        }
        MaterialFunctionStatement::Return { expr } => {
            let expr = translate_expr(expr, nums, ctx_name)?;
            Ok(format!("  return {expr};\n"))
        }
        MaterialFunctionStatement::ForLoop { var, from, to, body } => {
            let from = translate_expr(from, nums, ctx_name)?;
            let to = translate_expr(to, nums, ctx_name)?;
            let mut s = format!(
                "  for (var {var}: i32 = i32({from}); {var} < i32({to}); {var} = {var} + 1) {{\n"
            );
            s.push_str(&format!("    let {var}_f: f32 = f32({var});\n"));
            for line in body {
                let translated = translate_stmt(line, nums, ctx_name)?;
                let translated = translated.replace(&format!("{var}"), &format!("{var}_f"));
                for l in translated.lines() {
                    s.push_str("    ");
                    s.push_str(l);
                    s.push('\n');
                }
            }
            s.push_str("  }\n");
            Ok(s)
        }
    }
}

fn translate_expr(expr: &Expr, nums: &HashMap<String, f32>, ctx_name: &str) -> Result<String, RenderError> {
    match expr {
        Expr::Number(v) => Ok(wgsl_f32(*v as f32)),
        Expr::Ident(name) => {
            if let Some(v) = nums.get(name) {
                return Ok(wgsl_f32(*v));
            }
            Ok(name.clone())
        }
        Expr::Unary { op: UnaryOp::Neg, expr } => Ok(format!("(-{})", translate_expr(expr, nums, ctx_name)?)),
        Expr::Binary { lhs, op, rhs } => {
            let lhs = translate_expr(lhs, nums, ctx_name)?;
            let rhs = translate_expr(rhs, nums, ctx_name)?;
            let op = match op {
                BinaryOp::Add => "+",
                BinaryOp::Sub => "-",
                BinaryOp::Mul => "*",
                BinaryOp::Div => "/",
                BinaryOp::Intersect => {
                    return Err(RenderError::Backend("GPU translation failed: intersect binary op unsupported in field shader".to_string()))
                }
            };
            Ok(format!("({lhs} {op} {rhs})"))
        }
        Expr::Member { .. } => {
            if let Some(mapped) = translate_ctx_member(expr, ctx_name) {
                Ok(mapped)
            } else {
                Err(RenderError::Backend("GPU translation failed: unsupported member access".to_string()))
            }
        }
        Expr::Call { callee, args } => {
            if let Expr::Ident(name) = callee.as_ref()
                && name == "sample"
                && args.len() == 3
                && let Expr::Ident(input_name) = &args[0]
            {
                let x = translate_expr(&args[1], nums, ctx_name)?;
                let z = translate_expr(&args[2], nums, ctx_name)?;
                return Ok(format!("sample_{input_name}({x}, {z})"));
            }
            let fn_name = match callee.as_ref() {
                Expr::Ident(n) if n == "eval" => "eval_field".to_string(),
                Expr::Ident(n) => n.clone(),
                _ => {
                    return Err(RenderError::Backend(
                        "GPU translation failed: unsupported call target".to_string(),
                    ));
                }
            };
            let args = args
                .iter()
                .map(|arg| {
                    if let Some(mapped) = translate_ctx_member(arg, ctx_name) {
                        Ok(mapped)
                    } else {
                        translate_expr(arg, nums, ctx_name)
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(format!("{fn_name}({})", args.join(", ")))
        }
        Expr::String(_) | Expr::Array(_) | Expr::FunctionLiteral { .. } | Expr::ObjectLiteral { .. } => Err(
            RenderError::Backend("GPU translation failed: unsupported expression in field shader".to_string()),
        ),
    }
}

fn translate_ctx_member(expr: &Expr, ctx_name: &str) -> Option<String> {
    let Expr::Member { target, field } = expr else {
        return None;
    };
    let Expr::Member { target: pos_target, field: pos_field } = target.as_ref() else {
        return None;
    };
    let Expr::Ident(base) = pos_target.as_ref() else {
        return None;
    };
    if base != ctx_name || pos_field != "pos2d" {
        return None;
    }
    match field.as_str() {
        "x" => Some(format!("{ctx_name}_x")),
        "z" => Some(format!("{ctx_name}_z")),
        "y" => Some("0.0".to_string()),
        _ => None,
    }
}

fn collect_sample_input_names_stmt(stmt: &MaterialFunctionStatement, out: &mut HashSet<String>) {
    match stmt {
        MaterialFunctionStatement::Binding { expr, .. } => collect_sample_input_names_expr(expr, out),
        MaterialFunctionStatement::Return { expr } => collect_sample_input_names_expr(expr, out),
        MaterialFunctionStatement::ForLoop { from, to, body, .. } => {
            collect_sample_input_names_expr(from, out);
            collect_sample_input_names_expr(to, out);
            for line in body {
                collect_sample_input_names_stmt(line, out);
            }
        }
    }
}

fn collect_sample_input_names_expr(expr: &Expr, out: &mut HashSet<String>) {
    match expr {
        Expr::Call { callee, args } => {
            if let Expr::Ident(name) = callee.as_ref()
                && name == "sample"
                && args.len() == 3
                && let Expr::Ident(input_name) = &args[0]
            {
                out.insert(input_name.clone());
            }
            collect_sample_input_names_expr(callee, out);
            for arg in args {
                collect_sample_input_names_expr(arg, out);
            }
        }
        Expr::Binary { lhs, rhs, .. } => {
            collect_sample_input_names_expr(lhs, out);
            collect_sample_input_names_expr(rhs, out);
        }
        Expr::Unary { expr, .. } => collect_sample_input_names_expr(expr, out),
        Expr::Member { target, .. } => collect_sample_input_names_expr(target, out),
        Expr::Array(items) => {
            for item in items {
                collect_sample_input_names_expr(item, out);
            }
        }
        Expr::ObjectLiteral { fields, .. } => {
            for (_, v) in fields {
                collect_sample_input_names_expr(v, out);
            }
        }
        Expr::Number(_) | Expr::String(_) | Expr::Ident(_) | Expr::FunctionLiteral { .. } => {}
    }
}

fn render_field_compute(
    shader_src: &str,
    input_buffers_host: &[Vec<f32>],
    width: u32,
    height: u32,
    world_size: f32,
) -> Result<Vec<f32>, RenderError> {
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
    }))
    .ok_or_else(|| RenderError::Backend("wgpu adapter not found".to_string()))?;

    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            label: Some("forgedthoughts-field-device"),
        },
        None,
    ))
    .map_err(|err| RenderError::Backend(format!("wgpu device creation failed: {err}")))?;

    let params = FieldParams {
        width,
        height,
        world_size,
        _pad: 0.0,
    };
    let element_count = (width as u64) * (height as u64);
    let byte_len = element_count * std::mem::size_of::<f32>() as u64;

    let mut gpu_inputs = Vec::new();
    for (idx, host) in input_buffers_host.iter().enumerate() {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("field-input-buffer-{idx}")),
            size: byte_len,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&buffer, 0, bytemuck::cast_slice(host));
        gpu_inputs.push(buffer);
    }

    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("field-output-buffer"),
        size: byte_len,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("field-readback-buffer"),
        size: byte_len,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("field-params-buffer"),
        size: std::mem::size_of::<FieldParams>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&params_buffer, 0, bytemuck::bytes_of(&params));

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("field-ast-shader"),
        source: wgpu::ShaderSource::Wgsl(shader_src.into()),
    });

    let mut entries = Vec::new();
    for i in 0..gpu_inputs.len() {
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: i as u32,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        });
    }
    entries.push(wgpu::BindGroupLayoutEntry {
        binding: gpu_inputs.len() as u32,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: false },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    });
    entries.push(wgpu::BindGroupLayoutEntry {
        binding: (gpu_inputs.len() + 1) as u32,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("field-bind-layout"),
        entries: &entries,
    });

    let mut bind_entries = Vec::new();
    for (i, input) in gpu_inputs.iter().enumerate() {
        bind_entries.push(wgpu::BindGroupEntry {
            binding: i as u32,
            resource: input.as_entire_binding(),
        });
    }
    bind_entries.push(wgpu::BindGroupEntry {
        binding: gpu_inputs.len() as u32,
        resource: output_buffer.as_entire_binding(),
    });
    bind_entries.push(wgpu::BindGroupEntry {
        binding: (gpu_inputs.len() + 1) as u32,
        resource: params_buffer.as_entire_binding(),
    });
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("field-bind-group"),
        layout: &layout,
        entries: &bind_entries,
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("field-pipeline-layout"),
        bind_group_layouts: &[&layout],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("field-pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: "main",
        compilation_options: wgpu::PipelineCompilationOptions::default(),
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("field-encoder"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("field-pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.dispatch_workgroups(width.div_ceil(8), height.div_ceil(8), 1);
    }
    encoder.copy_buffer_to_buffer(&output_buffer, 0, &readback_buffer, 0, byte_len);
    queue.submit(std::iter::once(encoder.finish()));

    let slice = readback_buffer.slice(..);
    let (tx, rx) = mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    device.poll(wgpu::Maintain::Wait);
    match rx.recv() {
        Ok(Ok(())) => {}
        Ok(Err(err)) => {
            return Err(RenderError::Backend(format!(
                "wgpu readback mapping failed: {err}"
            )));
        }
        Err(_) => return Err(RenderError::Backend("wgpu readback channel closed".to_string())),
    }

    let data = slice.get_mapped_range();
    let values = bytemuck::cast_slice::<u8, f32>(&data).to_vec();
    drop(data);
    readback_buffer.unmap();
    Ok(values)
}

fn wgsl_f32(v: f32) -> String {
    if !v.is_finite() {
        return "0.0".to_string();
    }
    let mut s = format!("{v}");
    if !s.contains('.') && !s.contains('e') && !s.contains('E') {
        s.push_str(".0");
    }
    s
}
