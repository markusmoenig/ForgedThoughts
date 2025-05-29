use crate::prelude::*;
use rustc_hash::FxHashMap;
use vek::{Vec2, Vec3, Vec4};

use super::NodeRole;

#[derive(Debug, Clone)]
pub struct RenderContext {
    pub uv: Vec2<F>,
    pub screen_size: Vec2<F>,
    pub world_pos: Vec3<F>,

    pub outputs: Vec<Vec4<f32>>,
}

#[derive(Debug, Clone)]
pub struct ParsedNode {
    pub name: String,
    pub role: NodeRole,
    pub domain: NodeDomain,
    pub node_type: String,
    pub params: FxHashMap<String, TerminalInput>,
    pub line_number: usize,
    pub is_output: bool,
    pub node_index: usize,
}

#[derive(Debug, Clone)]
pub enum TerminalInput {
    Value(NodeTerminalRole),
    Reference {
        node_index: usize,
        terminal: NodeTerminal,
    },
}

pub struct Graph {
    pub nodesink: Vec<Box<dyn Node>>,
    pub node_map: FxHashMap<String, usize>,

    pub shapes: Vec<usize>,

    parsed_nodes: Vec<ParsedNode>,
}

impl Default for Graph {
    fn default() -> Self {
        Self::new()
    }
}

impl Graph {
    pub fn new() -> Self {
        Self {
            nodesink: vec![],
            node_map: FxHashMap::default(),

            shapes: vec![],

            parsed_nodes: vec![],
        }
    }

    pub fn install(&mut self) {
        // Nodes
        let node = crate::node::nodes::value2d::ValueNoise2D::new();
        self.node_map
            .insert(node.name().into(), self.nodesink.len());
        self.nodesink.push(Box::new(node));

        let node = crate::node::nodes::mul::Mul::new();
        self.node_map
            .insert(node.name().into(), self.nodesink.len());
        self.nodesink.push(Box::new(node));

        let node = crate::node::nodes::point3d::Point3D::new();
        self.node_map
            .insert(node.name().into(), self.nodesink.len());
        self.nodesink.push(Box::new(node));

        let node = crate::node::nodes::output::Output::new();
        self.node_map
            .insert(node.name().into(), self.nodesink.len());
        self.nodesink.push(Box::new(node));

        // Shapes
        let node = crate::node::shapes::sphere::Sphere::new();
        self.node_map
            .insert(node.name().into(), self.nodesink.len());
        self.nodesink.push(Box::new(node));
    }

    pub fn compile(&mut self, source: String) -> Result<(), String> {
        self.parsed_nodes = self.parse_node_graph(&source)?;

        // Collect all shapes in the graph
        self.shapes = self
            .parsed_nodes
            .iter()
            .enumerate()
            .filter_map(|(i, node)| {
                if node.role == NodeRole::Shape {
                    Some(i)
                } else {
                    None
                }
            })
            .collect();

        Ok(())
    }

    /// Evaluates the shape distances for the given world position.
    pub fn evaluate_shapes(&self, pos: Vec3<f32>) -> F {
        let mut ctx = RenderContext {
            uv: Vec2::zero(),
            screen_size: Vec2::zero(),
            world_pos: pos,
            outputs: vec![Vec4::zero(); self.parsed_nodes.len()],
        };

        let mut min_dist = f32::MAX;
        // let mut hit_shape: Option<&ParsedNode> = None;

        for index in &self.shapes {
            self.evaluate_at(*index, &mut ctx);
            let v4 = ctx.outputs[*index];
            // println!("{}", v4);
            if v4.x < min_dist {
                min_dist = v4.x;
                // hit_shape = Some(parsed_node);
            }
        }

        min_dist
    }

    /// Executes the graph.
    pub fn evaluate_color(&self, x: usize, y: usize, screen_size: Vec2<F>) -> Vec4<F> {
        if self.parsed_nodes.is_empty() {
            return Vec4::zero();
        }

        let uv = Vec2::new(
            x as F / screen_size.x,
            (screen_size.y - y as F) / screen_size.y,
        );

        let mut ctx = RenderContext {
            uv,
            screen_size,
            world_pos: Vec3::zero(),
            outputs: vec![Vec4::zero(); self.parsed_nodes.len()],
        };

        self.evaluate_at(self.parsed_nodes.len() - 1, &mut ctx)
    }

    /// Recursively execute the nodes at the given index.
    fn evaluate_at(&self, index: usize, ctx: &mut RenderContext) -> Vec4<F> {
        if let Some(parsed) = self.parsed_nodes.get(index) {
            if let Some(node) = self.nodesink.get(parsed.node_index) {
                let mut args: Vec<Vec4<F>> = vec![];

                for input in &node.inputs() {
                    let value = match parsed.params.get(&input.name) {
                        Some(TerminalInput::Value(v)) => v.clone(),

                        Some(TerminalInput::Reference {
                            node_index,
                            terminal,
                        }) => {
                            // Recursively compute dependency
                            self.evaluate_at(*node_index, ctx);

                            let out = ctx.outputs[*node_index];
                            let extracted = terminal.role.extract_from_vec4(out, &terminal.swizzle);
                            extracted.coerce_to(input.role.len())
                        }

                        None => input.role.clone(),
                    };

                    args.push(value.to_vec4());
                }

                if self.parsed_nodes[index].domain == NodeDomain::D3 {
                    ctx.outputs[index] = node.evaluate_3d(ctx.world_pos, &args);
                } else {
                    ctx.outputs[index] = node.evaluate_2d(ctx.uv, ctx.screen_size, &args);
                }
            }
        }

        ctx.outputs[index]
    }

    /// Parses the graph and verifies node connections.
    pub fn parse_node_graph(&self, source: &str) -> Result<Vec<ParsedNode>, String> {
        let mut parsed_nodes = Vec::new();
        let mut current_node: Option<ParsedNode> = None;

        for (line_number, line) in source.lines().enumerate() {
            let line_number = line_number + 1;
            let trimmed = line.trim();

            if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with('#') {
                continue;
            }

            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                if let Some(node) = current_node.take() {
                    parsed_nodes.push(node);
                }

                if let Some((name, node_type)) = trimmed[1..trimmed.len() - 1].split_once(':') {
                    let node_type = node_type.trim();
                    if node_type != "Output" && !self.node_map.contains_key(node_type) {
                        return Err(format!(
                            "Unknown node type '{}' at line {}",
                            node_type,
                            line_number + 1
                        ));
                    }

                    if let Some(node_index) = self.node_map.get(node_type) {
                        current_node = Some(ParsedNode {
                            name: name.trim().to_string(),
                            role: self.nodesink[*node_index].role(),
                            domain: self.nodesink[*node_index].domain(),
                            node_type: node_type.to_string(),
                            params: FxHashMap::default(),
                            line_number: line_number + 1,
                            is_output: node_type == "Output",
                            node_index: *node_index,
                        });
                    }
                } else {
                    return Err(format!(
                        "Malformed node header at line {}: '{}'",
                        line_number + 1,
                        trimmed
                    ));
                }
            } else if let Some((key, value)) = trimmed.split_once('=') {
                if let Some(node) = current_node.as_mut() {
                    let key = key.trim().to_string();
                    let value = value.trim_start();

                    let input = if value.starts_with("vec4") {
                        TerminalInput::Value(NodeTerminalRole::Vec4(crate::utils::parse_vec4(
                            value,
                        )))
                    } else if value.starts_with("vec3") {
                        TerminalInput::Value(NodeTerminalRole::Vec3(crate::utils::parse_vec3(
                            value,
                        )))
                    } else if value.starts_with("vec2") {
                        TerminalInput::Value(NodeTerminalRole::Vec2(crate::utils::parse_vec2(
                            value,
                        )))
                    } else if let Ok(v) = value.parse::<F>() {
                        TerminalInput::Value(NodeTerminalRole::Vec1(v))
                    } else if value.contains('.') {
                        let parts: Vec<&str> = value.split('.').collect();
                        if parts.len() == 2 {
                            let ref_node = parts[0].trim();
                            let output_name = parts[1].trim();

                            if let Some((index, node)) = parsed_nodes
                                .iter()
                                .enumerate()
                                .find(|(_, n)| n.name == ref_node)
                            {
                                if let Some(node_index) = self.node_map.get(&node.node_type) {
                                    // Validate that the output_name exists in the referenced node's outputs
                                    if let Some(def_node) = self.nodesink.get(*node_index) {
                                        let output_exists = def_node
                                            .outputs()
                                            .iter()
                                            .any(|t| t.name == output_name);

                                        if !output_exists {
                                            return Err(format!(
                                            "Invalid reference '{}': output '{}' not found in node type '{}' at line {}",
                                            value, output_name, node.node_type, line_number
                                        ));
                                        }

                                        TerminalInput::Reference {
                                        node_index: index,
                                        terminal: def_node
                                            .outputs()
                                            .iter()
                                            .find(|t| t.name == output_name)
                                            .cloned()
                                            .ok_or_else(|| format!(
                                                "Internal error: output '{}' not found in node type '{}' after validation",
                                                output_name, node.node_type
                                            ))?,
                                    }
                                    } else {
                                        return Err(format!(
                                            "Unknown node type '{}' for node '{}' at line {}",
                                            node.node_type, node.name, line_number
                                        ));
                                    }
                                } else {
                                    return Err(format!(
                                        "Unknown node type '{}' for node '{}' at line {}",
                                        node.node_type, node.name, line_number
                                    ));
                                }
                            } else {
                                return Err(format!(
                                    "Invalid reference '{}': node '{}' not found at line {}",
                                    value, ref_node, line_number
                                ));
                            }
                        } else {
                            return Err(format!(
                                "Invalid reference format '{}' at line {} (expected format: node.output)",
                                value, line_number
                            ));
                        }
                    } else {
                        // fallback or error
                        return Err(format!(
                            "Invalid parameter value '{}' at line {}",
                            value, line_number
                        ));
                    };

                    node.params.insert(key, input);
                } else {
                    return Err(format!(
                        "Parameter found outside of a node definition at line {}: '{}'",
                        line_number + 1,
                        trimmed
                    ));
                }
            } else {
                return Err(format!(
                    "Unrecognized line format at line {}: '{}'",
                    line_number + 1,
                    trimmed
                ));
            }
        }

        if let Some(node) = current_node {
            parsed_nodes.push(node);
        }

        Ok(parsed_nodes)
    }
}
