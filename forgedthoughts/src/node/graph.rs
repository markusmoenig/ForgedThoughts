use crate::{Color, Node, NodeExecutionCtx, NodeTerminalRole, F};
use rustc_hash::FxHashMap;
use std::collections::VecDeque;
use vek::{Vec2, Vec4};
use wasmer::Value;

#[derive(Debug, Clone)]
pub struct ParsedNode {
    pub name: String,
    pub node_type: String,
    pub params: FxHashMap<String, TerminalInput>,
    pub line_number: usize,
    pub is_output: bool,
}

#[derive(Debug, Clone)]
pub enum TerminalInput {
    Value(NodeTerminalRole),
    Reference {
        node_index: usize,
        output_name: String,
    },
}

pub struct Graph {
    parsed_nodes: Vec<ParsedNode>,
    sorted_nodes: Vec<ParsedNode>,
}

impl Default for Graph {
    fn default() -> Self {
        Self::new()
    }
}

impl Graph {
    pub fn new() -> Self {
        Self {
            parsed_nodes: vec![],
            sorted_nodes: vec![],
        }
    }

    pub fn compile(
        &mut self,
        source: String,
        nodes: &FxHashMap<String, Node>,
    ) -> Result<(), String> {
        self.parsed_nodes = self.parse_node_graph(&source, nodes)?;
        self.sorted_nodes = self.sort_nodes()?;
        println!("{:?}", self.parsed_nodes);

        Ok(())
    }

    /// Executes the graph.
    pub fn execute(
        &self,
        x: usize,
        y: usize,
        screen_size: Vec2<F>,
        nodes: &FxHashMap<String, Node>,
        node_execution_ctx: &mut NodeExecutionCtx,
    ) -> Color {
        let mut node_outputs: Vec<Vec4<F>> = vec![Vec4::zero(); self.sorted_nodes.len()];
        let uv = Vec2::new(
            x as F / screen_size.x,
            (screen_size.y - y as F) / screen_size.y,
        );

        let mut last_executed = 0_usize;

        for (idx, parsed_node) in self.sorted_nodes.iter().enumerate() {
            let Some(node_def) = nodes.get(&parsed_node.node_type) else {
                continue;
            };

            let mut args = crate::utils::build_uv_args(uv, screen_size);

            for input_def in &node_def.inputs {
                let input = parsed_node.params.get(&input_def.name);

                let value = match input {
                    Some(TerminalInput::Value(v)) => v.clone(),
                    Some(TerminalInput::Reference {
                        node_index,
                        output_name,
                    }) => {
                        // let out_vec4 = node_outputs.get(*node_index).copied().unwrap_or_default();

                        // // Get the actual output terminal from the referenced node
                        // if let Some(output_terminal) =
                        //     node_def.outputs.iter().find(|t| t.name == *output_name)
                        // {
                        //     // Extract the correct value from the vec4 using the referenced terminal's swizzle
                        //     output_terminal
                        //         .role
                        //         .extract_from_vec4(out_vec4, &output_terminal.swizzle)
                        // } else {
                        //     NodeTerminalRole::Vec1(0.0) // fallback if output not found
                        // }
                        let out = node_outputs.get(*node_index).copied().unwrap_or_default();
                        input_def.role.extract_from_vec4(out, &input_def.swizzle)
                    }
                    None => input_def.role.clone(),
                };

                // println!("{:?}", value);
                crate::utils::push_terminal_value(&mut args, &value);
            }

            if let Some((_, instance)) = node_execution_ctx
                .modules_instances
                .get_mut(&parsed_node.node_type)
            {
                if let Ok(func) = instance.exports.get_function("main") {
                    if let Ok(results) = func.call(&mut node_execution_ctx.store, &args) {
                        node_outputs[idx] = crate::utils::values_to_vec4(&results);
                        last_executed = idx;
                    }
                }
            }
        }

        // node_outputs[last_executed].w = 1.0;
        // println!("{:?}", node_outputs[last_executed]);
        // return [uv.x, uv.y, 0.0, 1.0];
        return node_outputs[last_executed].into_array();

        let mut result: Color = [0.0, 0.0, 0.0, 0.0];

        if let Some((_module, instance)) =
            node_execution_ctx.modules_instances.get_mut("ValueNoise2D")
        {
            if let Ok(func) = instance.exports.get_function("main") {
                #[cfg(feature = "double")]
                let mut args = vec![
                    Value::F64(x as f64 / screen_size.x as f64),
                    Value::F64((screen_size.y as f64 - y as f64) / screen_size.y as f64),
                    Value::F64(screen_size.x as f64),
                    Value::F64(screen_size.y as f64),
                ];

                #[cfg(not(feature = "double"))]
                let mut args = vec![
                    Value::F32(x as f32 / screen_size.x),
                    Value::F32((screen_size.y - y as f32) / screen_size.y),
                    Value::F32(screen_size.x),
                    Value::F32(screen_size.x),
                ];

                if let Some(node) = nodes.get("ValueNoise2D") {
                    node.add_parameters(&mut args, FxHashMap::default());
                }

                if let Ok(values) = func.call(&mut node_execution_ctx.store, &args) {
                    result = crate::utils::values_to_array4(&values);
                }
            }
        }

        result
    }

    /// Sort the parsed nodes based on dependencies.
    pub fn sort_nodes(&self) -> Result<Vec<ParsedNode>, String> {
        let mut in_degrees = vec![0; self.parsed_nodes.len()];
        let mut graph = vec![vec![]; self.parsed_nodes.len()];

        // Build dependency graph
        for (i, node) in self.parsed_nodes.iter().enumerate() {
            for input in node.params.values() {
                if let TerminalInput::Reference { node_index, .. } = input {
                    graph[*node_index].push(i);
                    in_degrees[i] += 1;
                }
            }
        }

        // Kahn's algorithm
        let mut queue: VecDeque<usize> = in_degrees
            .iter()
            .enumerate()
            .filter(|&(_, &deg)| deg == 0)
            .map(|(i, _)| i)
            .collect();

        let mut sorted = Vec::with_capacity(self.parsed_nodes.len());

        while let Some(i) = queue.pop_front() {
            sorted.push(self.parsed_nodes[i].clone());
            for &dependent in &graph[i] {
                in_degrees[dependent] -= 1;
                if in_degrees[dependent] == 0 {
                    queue.push_back(dependent);
                }
            }
        }

        if sorted.len() != self.parsed_nodes.len() {
            return Err("Cycle detected in node graph".to_string());
        }

        Ok(sorted)
    }

    /// Parses the graph and verifies node connections.
    pub fn parse_node_graph(
        &self,
        source: &str,
        known_nodes: &FxHashMap<String, Node>,
    ) -> Result<Vec<ParsedNode>, String> {
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
                    if node_type != "Output" && !known_nodes.contains_key(node_type) {
                        return Err(format!(
                            "Unknown node type '{}' at line {}",
                            node_type,
                            line_number + 1
                        ));
                    }

                    current_node = Some(ParsedNode {
                        name: name.trim().to_string(),
                        node_type: node_type.to_string(),
                        params: FxHashMap::default(),
                        line_number: line_number + 1,
                        is_output: node_type == "Output",
                    });
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
                                // Validate that the output_name exists in the referenced node's outputs
                                if let Some(def_node) = known_nodes.get(&node.node_type) {
                                    let output_exists =
                                        def_node.outputs.iter().any(|t| t.name == output_name);

                                    if !output_exists {
                                        return Err(format!(
                                            "Invalid reference '{}': output '{}' not found in node type '{}' at line {}",
                                            value, output_name, node.node_type, line_number
                                        ));
                                    }

                                    TerminalInput::Reference {
                                        node_index: index,
                                        output_name: output_name.to_string(),
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
