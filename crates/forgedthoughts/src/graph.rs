use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    fs,
    path::Path,
};

use toml::Value as TomlValue;

use crate::{CoreError, builtin_library_item_metadata, builtin_library_items};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GraphPortType {
    Scalar,
}

#[derive(Debug, Clone)]
struct GraphNodeSchema {
    inputs: BTreeMap<String, GraphPortType>,
    params: BTreeSet<String>,
    outputs: BTreeMap<String, GraphPortType>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GraphRenderConfig {
    pub stage: String,
    pub target: String,
    pub source: String,
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
    let mut nodes = BTreeMap::new();

    for (key, value) in table {
        if key == "version" || key == "render" {
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

    Ok(GraphFile { render, nodes })
}

pub fn lower_graph_to_ft(graph: &GraphFile) -> Result<String, CoreError> {
    if graph.render.stage != "height" {
        return Err(CoreError::Graph(format!(
            "unsupported render stage '{}'; only 'height' is supported right now",
            graph.render.stage
        )));
    }
    if graph.render.target != "grayscale" {
        return Err(CoreError::Graph(format!(
            "unsupported render target '{}'; only 'grayscale' is supported right now",
            graph.render.target
        )));
    }

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
    })
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
    if render.stage != "height" {
        return Err(CoreError::Graph(format!(
            "unsupported render stage '{}'; only 'height' is supported right now",
            render.stage
        )));
    }
    if render.target != "grayscale" {
        return Err(CoreError::Graph(format!(
            "unsupported render target '{}'; only 'grayscale' is supported right now",
            render.target
        )));
    }
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
    if render_port_ty != GraphPortType::Scalar {
        return Err(CoreError::Graph(format!(
            "render source '{}' must resolve to a scalar output for target '{}'",
            render.source, render.target
        )));
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
) -> Result<GraphPortType, CoreError> {
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
) -> Result<GraphPortType, CoreError> {
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

fn parse_port_type(value: &str) -> Result<GraphPortType, CoreError> {
    match value {
        "scalar" => Ok(GraphPortType::Scalar),
        _ => Err(CoreError::Graph(format!(
            "unsupported graph port type '{value}'"
        ))),
    }
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
