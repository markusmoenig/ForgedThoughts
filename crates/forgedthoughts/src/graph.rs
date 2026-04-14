use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    fs,
    path::Path,
};

use toml::Value as TomlValue;

use crate::{CoreError, builtin_library_item_metadata, builtin_library_items};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphRenderSourceKind {
    PointScalar,
    FieldScalar,
}

#[derive(Debug, Clone)]
struct GraphNodeSchema {
    inputs: BTreeMap<String, GraphRenderSourceKind>,
    params: BTreeSet<String>,
    outputs: BTreeMap<String, GraphRenderSourceKind>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GraphRenderConfig {
    pub stage: String,
    pub target: String,
    pub source: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// Camera settings parsed from a `[camera]` TOML table.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphCamera {
    /// Eye position in world space.
    pub origin: [f32; 3],
    /// Look-at target.
    pub target: [f32; 3],
    /// Up vector (default `[0, 1, 0]`).
    pub up: [f32; 3],
    /// Vertical field of view in degrees.
    pub fov_y: f32,
}

impl Default for GraphCamera {
    fn default() -> Self {
        Self {
            origin: [0.5, 1.5, -2.0],
            target: [0.5, 0.0, 0.5],
            up: [0.0, 1.0, 0.0],
            fov_y: 45.0,
        }
    }
}

/// Sun / directional light settings from a `[sun]` TOML table.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphSun {
    /// Direction *toward* the sun (will be normalised).
    pub direction: [f32; 3],
    /// RGB radiance/intensity.
    pub color: [f32; 3],
    /// Intensity multiplier.
    pub intensity: f32,
}

impl Default for GraphSun {
    fn default() -> Self {
        Self {
            direction: [0.6, 1.0, 0.4],
            color: [1.0, 0.95, 0.85],
            intensity: 3.0,
        }
    }
}

/// Sky ambient light settings from a `[sky]` TOML table.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphSky {
    pub color: [f32; 3],
    pub intensity: f32,
}

impl Default for GraphSky {
    fn default() -> Self {
        Self {
            color: [0.4, 0.55, 0.8],
            intensity: 0.4,
        }
    }
}

/// Scene-render settings parsed from a `[scene]` TOML table.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphSceneSettings {
    /// Maximum raymarching steps.
    pub max_steps: u32,
    /// Maximum ray distance.
    pub max_dist: f32,
    /// Height scale: world-space height range is `[0, height_scale]`.
    pub height_scale: f32,
    /// World tile size in world units (terrain spans `[0, world_size]` in X and Z).
    pub world_size: f32,
    /// Epsilon for surface detection.
    pub epsilon: f32,
    /// Normal estimation finite-difference step.
    pub normal_eps: f32,
}

impl Default for GraphSceneSettings {
    fn default() -> Self {
        Self {
            max_steps: 256,
            max_dist: 20.0,
            height_scale: 1.0,
            world_size: 1.0,
            epsilon: 0.001,
            normal_eps: 0.002,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GraphNodeInstance {
    pub node_type: String,
    pub alias: String,
    pub fields: BTreeMap<String, GraphValue>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GraphValue {
    Number(f64),
    String(String),
    Array(Vec<GraphValue>),
    Ref { instance: String, port: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct GraphFile {
    pub render: GraphRenderConfig,
    pub nodes: BTreeMap<String, GraphNodeInstance>,
    pub camera: GraphCamera,
    pub sun: GraphSun,
    pub sky: GraphSky,
    pub scene: GraphSceneSettings,
}

pub fn load_graph_file(path: &Path) -> Result<GraphFile, CoreError> {
    let source = fs::read_to_string(path).map_err(|source| CoreError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    parse_graph_file(&source)
}

pub fn parse_graph_file(source: &str) -> Result<GraphFile, CoreError> {
    let value: TomlValue = source.parse().map_err(CoreError::GraphToml)?;
    let table = value
        .as_table()
        .ok_or_else(|| CoreError::Graph("graph root must be a TOML table".to_string()))?;

    let render = parse_render(table.get("render"))?;
    let camera = parse_camera(table.get("camera"));
    let sun = parse_sun(table.get("sun"));
    let sky = parse_sky(table.get("sky"));
    let scene = parse_scene_settings(table.get("scene"));
    let mut nodes = BTreeMap::new();

    // Reserved top-level keys that are not node instance groups.
    let reserved = ["version", "render", "camera", "sun", "sky", "scene"];

    for (key, value) in table {
        if reserved.contains(&key.as_str()) {
            continue;
        }

        let aliases = value.as_table().ok_or_else(|| {
            CoreError::Graph(format!(
                "graph table '{key}' must contain alias subtables from [type.alias] entries"
            ))
        })?;

        for (alias, alias_value) in aliases {
            let id = format!("{key}.{alias}");
            let fields = parse_node_fields(&id, alias_value)?;
            nodes.insert(
                id.clone(),
                GraphNodeInstance {
                    node_type: key.clone(),
                    alias: alias.clone(),
                    fields,
                },
            );
        }
    }

    if nodes.is_empty() {
        return Err(CoreError::Graph("graph file defines no node instances".to_string()));
    }

    validate_graph(&render, &nodes)?;

    Ok(GraphFile { render, nodes, camera, sun, sky, scene })
}

pub fn lower_graph_to_ft(graph: &GraphFile) -> Result<String, CoreError> {
    validate_render_mode(&graph.render)?;

    let (instance, port) = parse_ref(&graph.render.source)?;
    if port != "field" {
        return Err(CoreError::Graph(format!(
            "unsupported render source port '{port}'; only 'field' is supported right now"
        )));
    }

    let mut imports = BTreeSet::new();
    let object_expr = lower_instance_to_ft(graph, &instance, &mut Vec::new(), &mut imports)?;
    let mut out = String::new();
    for import in imports {
        out.push_str(&format!("import \"{import}\";\n"));
    }
    out.push_str("\n");
    out.push_str(&format!("let graph = {object_expr};\n"));
    Ok(out)
}

fn parse_render(value: Option<&TomlValue>) -> Result<GraphRenderConfig, CoreError> {
    let table = value
        .and_then(TomlValue::as_table)
        .ok_or_else(|| CoreError::Graph("graph file requires a [render] table".to_string()))?;
    Ok(GraphRenderConfig {
        stage: required_string(table, "stage", "render")?,
        target: required_string(table, "target", "render")?,
        source: required_string(table, "source", "render")?,
        width: optional_u32(table, "width"),
        height: optional_u32(table, "height"),
    })
}

fn parse_camera(value: Option<&TomlValue>) -> GraphCamera {
    let Some(table) = value.and_then(TomlValue::as_table) else {
        return GraphCamera::default();
    };
    let mut cam = GraphCamera::default();
    if let Some(v) = optional_f32_array3(table, "origin") { cam.origin = v; }
    if let Some(v) = optional_f32_array3(table, "target") { cam.target = v; }
    if let Some(v) = optional_f32_array3(table, "up") { cam.up = v; }
    if let Some(v) = optional_f32(table, "fov_y") { cam.fov_y = v; }
    cam
}

fn parse_sun(value: Option<&TomlValue>) -> GraphSun {
    let Some(table) = value.and_then(TomlValue::as_table) else {
        return GraphSun::default();
    };
    let mut sun = GraphSun::default();
    if let Some(v) = optional_f32_array3(table, "direction") { sun.direction = v; }
    if let Some(v) = optional_f32_array3(table, "color") { sun.color = v; }
    if let Some(v) = optional_f32(table, "intensity") { sun.intensity = v; }
    sun
}

fn parse_sky(value: Option<&TomlValue>) -> GraphSky {
    let Some(table) = value.and_then(TomlValue::as_table) else {
        return GraphSky::default();
    };
    let mut sky = GraphSky::default();
    if let Some(v) = optional_f32_array3(table, "color") { sky.color = v; }
    if let Some(v) = optional_f32(table, "intensity") { sky.intensity = v; }
    sky
}

fn parse_scene_settings(value: Option<&TomlValue>) -> GraphSceneSettings {
    let Some(table) = value.and_then(TomlValue::as_table) else {
        return GraphSceneSettings::default();
    };
    let mut s = GraphSceneSettings::default();
    if let Some(v) = optional_u32(table, "max_steps") { s.max_steps = v; }
    if let Some(v) = optional_f32(table, "max_dist") { s.max_dist = v; }
    if let Some(v) = optional_f32(table, "height_scale") { s.height_scale = v; }
    if let Some(v) = optional_f32(table, "world_size") { s.world_size = v; }
    if let Some(v) = optional_f32(table, "epsilon") { s.epsilon = v; }
    if let Some(v) = optional_f32(table, "normal_eps") { s.normal_eps = v; }
    s
}

fn optional_f32(table: &toml::map::Map<String, TomlValue>, key: &str) -> Option<f32> {
    match table.get(key)? {
        TomlValue::Float(v) => Some(*v as f32),
        TomlValue::Integer(v) => Some(*v as f32),
        _ => None,
    }
}

fn optional_u32(table: &toml::map::Map<String, TomlValue>, key: &str) -> Option<u32> {
    match table.get(key)? {
        TomlValue::Integer(v) => Some((*v).max(0) as u32),
        TomlValue::Float(v) => Some(*v as u32),
        _ => None,
    }
}

fn optional_f32_array3(
    table: &toml::map::Map<String, TomlValue>,
    key: &str,
) -> Option<[f32; 3]> {
    let arr = table.get(key)?.as_array()?;
    if arr.len() != 3 { return None; }
    let x = to_f32(&arr[0])?;
    let y = to_f32(&arr[1])?;
    let z = to_f32(&arr[2])?;
    Some([x, y, z])
}

fn to_f32(v: &TomlValue) -> Option<f32> {
    match v {
        TomlValue::Float(f) => Some(*f as f32),
        TomlValue::Integer(i) => Some(*i as f32),
        _ => None,
    }
}

fn parse_node_fields(
    table_name: &str,
    value: &TomlValue,
) -> Result<BTreeMap<String, GraphValue>, CoreError> {
    let table = value.as_table().ok_or_else(|| {
        CoreError::Graph(format!("graph node table '{table_name}' must be a TOML table"))
    })?;
    let mut fields = BTreeMap::new();
    for (name, value) in table {
        fields.insert(name.clone(), parse_graph_value(name, value)?);
    }
    Ok(fields)
}

fn parse_graph_value(name: &str, value: &TomlValue) -> Result<GraphValue, CoreError> {
    match value {
        TomlValue::Integer(v) => Ok(GraphValue::Number(*v as f64)),
        TomlValue::Float(v) => Ok(GraphValue::Number(*v)),
        TomlValue::String(v) => {
            if let Ok((instance, port)) = parse_ref(v) {
                Ok(GraphValue::Ref { instance, port })
            } else {
                Ok(GraphValue::String(v.clone()))
            }
        }
        TomlValue::Array(items) => Ok(GraphValue::Array(
            items
                .iter()
                .map(|item| parse_graph_value(name, item))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        TomlValue::Boolean(_) => Err(CoreError::Graph(format!(
            "field '{name}' uses boolean values, which are not supported by FT node lowering yet"
        ))),
        _ => Err(CoreError::Graph(format!(
            "field '{name}' uses an unsupported TOML value kind"
        ))),
    }
}

fn validate_graph(
    render: &GraphRenderConfig,
    nodes: &BTreeMap<String, GraphNodeInstance>,
) -> Result<(), CoreError> {
    let (render_instance, render_port) = parse_ref(&render.source)?;
    validate_render_mode(render)?;
    let render_node = nodes.get(&render_instance).ok_or_else(|| {
        CoreError::Graph(format!(
            "render source references unknown node instance '{render_instance}'"
        ))
    })?;
    let render_schema = node_schema(&render_node.node_type)?;
    let render_port_ty = output_port_type(
        &render_schema,
        &render_port,
        &render_node.node_type,
        "render source",
    )?;
    match (render.stage.as_str(), render.target.as_str(), render_port_ty) {
        ("height", "grayscale", GraphRenderSourceKind::PointScalar)
        | ("height", "grayscale", GraphRenderSourceKind::FieldScalar)
        | ("scene", "raytrace", GraphRenderSourceKind::PointScalar)
        | ("scene", "raytrace", GraphRenderSourceKind::FieldScalar) => {}
        _ => {
            return Err(CoreError::Graph(format!(
                "render source '{}' is incompatible with render mode '{} / {}'",
                render.source, render.stage, render.target
            )))
        }
    }

    for node in nodes.values() {
        let schema = node_schema(&node.node_type)?;
        for (field, value) in &node.fields {
            if is_editor_field(field) {
                continue;
            }
            if let GraphValue::Ref { instance, port } = value {
                let input_ty = input_port_type(&schema, field, &node.node_type)?;
                let source_node = nodes.get(instance).ok_or_else(|| {
                    CoreError::Graph(format!(
                        "field '{}.{}' references unknown node instance '{instance}'",
                        node.node_type, field
                    ))
                })?;
                let source_schema = node_schema(&source_node.node_type)?;
                let output_ty = output_port_type(
                    &source_schema,
                    port,
                    &source_node.node_type,
                    &format!("field '{}.{}'", node.node_type, field),
                )?;
                if input_ty != output_ty {
                    return Err(CoreError::Graph(format!(
                        "field '{}.{}' expects a {:?} input but '{}:{}' is {:?}",
                        node.node_type, field, input_ty, instance, port, output_ty
                    )));
                }
            } else if !is_param_field(&schema, field) {
                return Err(CoreError::Graph(format!(
                    "field '{}.{}' is not a declared input/param for node type '{}'",
                    node.node_type, field, node.node_type
                )));
            }
        }
    }

    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    visit_graph(&render_instance, nodes, &mut visiting, &mut visited)?;

    Ok(())
}

fn visit_graph(
    instance: &str,
    nodes: &BTreeMap<String, GraphNodeInstance>,
    visiting: &mut HashSet<String>,
    visited: &mut HashSet<String>,
) -> Result<(), CoreError> {
    if visited.contains(instance) {
        return Ok(());
    }
    if !visiting.insert(instance.to_string()) {
        return Err(CoreError::Graph(format!(
            "graph contains a cycle at '{instance}'"
        )));
    }

    let node = nodes.get(instance).ok_or_else(|| {
        CoreError::Graph(format!("graph references unknown node instance '{instance}'"))
    })?;
    for value in node.fields.values() {
        if let GraphValue::Ref { instance, .. } = value {
            visit_graph(instance, nodes, visiting, visited)?;
        }
    }

    visiting.remove(instance);
    visited.insert(instance.to_string());
    Ok(())
}

fn lower_instance_to_ft(
    graph: &GraphFile,
    instance: &str,
    stack: &mut Vec<String>,
    imports: &mut BTreeSet<String>,
) -> Result<String, CoreError> {
    if stack.iter().any(|item| item == instance) {
        return Err(CoreError::Graph(format!(
            "graph contains a cycle at '{}'",
            stack.join(" -> ")
        )));
    }
    stack.push(instance.to_string());

    let node = graph.nodes.get(instance).ok_or_else(|| {
        CoreError::Graph(format!("graph references unknown node instance '{instance}'"))
    })?;
    imports.insert(node.node_type.clone());

    let mut out = String::new();
    out.push_str(&format!("{} {{\n", node.node_type));
    for (name, value) in &node.fields {
        let ft_value = match value {
            GraphValue::Ref { instance, port: _ } => {
                let schema = node_schema(&node.node_type)?;
                let _ = input_port_type(&schema, name, &node.node_type)?;
                lower_instance_to_ft(graph, instance, stack, imports)?
            }
            _ => graph_value_to_ft(value)?,
        };
        out.push_str(&format!("  {name}: {ft_value},\n"));
    }
    out.push('}');

    stack.pop();
    Ok(out)
}

fn node_schema(node_type: &str) -> Result<GraphNodeSchema, CoreError> {
    let item = builtin_library_items(None)
        .into_iter()
        .find(|item| item.name == node_type)
        .ok_or_else(|| {
            CoreError::Graph(format!(
                "node type '{node_type}' is not a known built-in graph node"
            ))
        })?;
    let metadata = builtin_library_item_metadata(&item);
    let inputs = metadata
        .inputs
        .into_iter()
        .map(|port| Ok((port.name, parse_port_type(&port.port_type)?)))
        .collect::<Result<BTreeMap<_, _>, CoreError>>()?;
    let params = metadata
        .params
        .into_iter()
        .map(|param| param.name)
        .collect::<BTreeSet<_>>();
    let outputs = metadata
        .outputs
        .into_iter()
        .map(|port| Ok((port.name, parse_port_type(&port.port_type)?)))
        .collect::<Result<BTreeMap<_, _>, CoreError>>()?;
    if outputs.is_empty() {
        return Err(CoreError::Graph(format!(
            "node type '{node_type}' is missing graph output metadata"
        )));
    }
    Ok(GraphNodeSchema {
        inputs,
        params,
        outputs,
    })
}

fn input_port_type(
    schema: &GraphNodeSchema,
    field: &str,
    node_type: &str,
) -> Result<GraphRenderSourceKind, CoreError> {
    schema
        .inputs
        .get(field)
        .copied()
        .ok_or_else(|| {
            CoreError::Graph(format!(
                "field '{field}' is not a declared input port on node type '{node_type}'"
            ))
        })
}

fn output_port_type(
    schema: &GraphNodeSchema,
    field: &str,
    node_type: &str,
    scope: &str,
) -> Result<GraphRenderSourceKind, CoreError> {
    schema
        .outputs
        .get(field)
        .copied()
        .ok_or_else(|| {
            CoreError::Graph(format!(
                "{scope} references unknown output port '{field}' on node type '{node_type}'"
            ))
        })
}

fn is_param_field(schema: &GraphNodeSchema, field: &str) -> bool {
    schema.params.contains(field)
}

fn is_editor_field(field: &str) -> bool {
    matches!(field, "pos")
}

fn parse_port_type(value: &str) -> Result<GraphRenderSourceKind, CoreError> {
    match value {
        "point_scalar" => Ok(GraphRenderSourceKind::PointScalar),
        "field_scalar" => Ok(GraphRenderSourceKind::FieldScalar),
        _ => Err(CoreError::Graph(format!(
            "unsupported graph port type '{value}'"
        ))),
    }
}

fn validate_render_mode(render: &GraphRenderConfig) -> Result<(), CoreError> {
    match (render.stage.as_str(), render.target.as_str()) {
        ("height", "grayscale") | ("scene", "raytrace") => Ok(()),
        ("height", target) => Err(CoreError::Graph(format!(
            "unsupported render target '{target}' for stage 'height'; supported: grayscale"
        ))),
        ("scene", target) => Err(CoreError::Graph(format!(
            "unsupported render target '{target}' for stage 'scene'; supported: raytrace"
        ))),
        (stage, _) => Err(CoreError::Graph(format!(
            "unsupported render stage '{stage}'; supported: height, scene"
        ))),
    }
}

pub fn graph_render_source_kind(graph: &GraphFile) -> Result<GraphRenderSourceKind, CoreError> {
    let (instance, port) = parse_ref(&graph.render.source)?;
    let node = graph.nodes.get(&instance).ok_or_else(|| {
        CoreError::Graph(format!(
            "render source references unknown node instance '{instance}'"
        ))
    })?;
    let schema = node_schema(&node.node_type)?;
    output_port_type(&schema, &port, &node.node_type, "render source")
}

fn graph_value_to_ft(value: &GraphValue) -> Result<String, CoreError> {
    match value {
        GraphValue::Number(v) => Ok(format_number(*v)),
        GraphValue::String(v) => Ok(format!("{v:?}")),
        GraphValue::Array(items) => Ok(format!(
            "[{}]",
            items
                .iter()
                .map(graph_value_to_ft)
                .collect::<Result<Vec<_>, _>>()?
                .join(", ")
        )),
        GraphValue::Ref { instance, port } => Err(CoreError::Graph(format!(
            "connections are not lowered yet: '{instance}:{port}'"
        ))),
    }
}

fn format_number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.1}")
    } else {
        value.to_string()
    }
}

fn required_string(
    table: &toml::map::Map<String, TomlValue>,
    key: &str,
    scope: &str,
) -> Result<String, CoreError> {
    table
        .get(key)
        .and_then(TomlValue::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| CoreError::Graph(format!("[{scope}] requires string field '{key}'")))
}

fn parse_ref(value: &str) -> Result<(String, String), CoreError> {
    let Some((instance, port)) = value.split_once(':') else {
        return Err(CoreError::Graph(format!(
            "invalid graph reference '{value}'; expected 'type.alias:port'"
        )));
    };
    let Some((_node_type, _alias)) = instance.split_once('.') else {
        return Err(CoreError::Graph(format!(
            "invalid graph reference '{value}'; expected 'type.alias:port'"
        )));
    };
    if port.is_empty() {
        return Err(CoreError::Graph(format!(
            "invalid graph reference '{value}'; expected 'type.alias:port'"
        )));
    }
    Ok((instance.to_string(), port.to_string()))
}
