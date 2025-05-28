pub mod graph;
pub mod terminal;

use crate::{NodeTerminal, NodeTerminalRole, Scanner, TokenType, F};
use regex::Regex;
use rustc_hash::FxHashMap;
use std::sync::Arc;
use vek::{Vec2, Vec3, Vec4};
use wasmer::{Instance, Module, Store, Value};

/// Holds the wasmer compiled nodes per thread.
pub struct NodeExecutionCtx {
    pub store: Store,
    pub modules_instances: FxHashMap<String, (Arc<Module>, Instance)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum NodeRole {
    Node,
    Shape,
}

pub struct Node {
    pub name: String,

    pub role: NodeRole,

    pub inputs: Vec<NodeTerminal>,
    pub outputs: Vec<NodeTerminal>,

    pub output: Vec4<F>,

    source: String,
    pub wat: String,
}

impl Default for Node {
    fn default() -> Self {
        Self::new()
    }
}

impl Node {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            role: NodeRole::Node,
            inputs: vec![],
            outputs: vec![],
            output: Vec4::zero(),
            source: String::new(),
            wat: String::new(),
        }
    }

    pub fn compile(&mut self, source: String, high_precision: bool) -> Result<(), String> {
        let mut scanner = Scanner::new(source.clone());

        let token = scanner.scan_token(false);
        if token.kind != TokenType::Identifier
            && (token.lexeme != "Node" || token.lexeme != "Shape")
        {
            return Err(format!(
                "Expected one of 'Node' or 'Shape' at line {}.",
                token.line
            ));
        }

        if token.lexeme == "Shape" {
            self.role = NodeRole::Shape;
        }

        let token = scanner.scan_token(false);
        if token.kind != TokenType::Less {
            return Err(format!(
                "Expected '<' after 'Node' identifier at line {}.",
                token.line
            ));
        }

        let token = scanner.scan_token(false);
        if token.kind != TokenType::Identifier {
            return Err(format!(
                "Expected node name after 'Node<' at line {}.",
                token.line
            ));
        } else {
            self.name = token.lexeme.clone();
        }

        let token = scanner.scan_token(false);
        if token.kind != TokenType::Greater {
            return Err(format!(
                "Expected '>' after node name identifier at line {}.",
                token.line
            ));
        }

        let (inputs, outputs) = self.parse_terminals(&source);
        self.inputs = inputs;
        self.outputs = outputs;
        // println!("{:?}, outputs: {:?}", self.inputs, self.outputs);

        // Get RPU source

        if let Some((_, rpu)) = source.split_once("Source<RPU>") {
            self.source = rpu.to_string();
            let rpu = rpu::RPU::new();
            match rpu.compile_to_wat(self.source.clone(), high_precision) {
                Ok(wat) => {
                    self.wat = wat;
                }
                Err(err) => return Err(format!("RPU: {}", err)),
            }
        }

        Ok(())
    }

    pub fn add_parameters(
        &self,
        params: &mut Vec<wasmer::Value>,
        inputs: FxHashMap<String, NodeTerminalRole>,
    ) {
        for terminal in &self.inputs {
            let role = inputs.get(&terminal.name).unwrap_or(&terminal.role);

            match role {
                NodeTerminalRole::Vec1(v) => {
                    #[cfg(feature = "double")]
                    params.push(Value::F64(*v));
                    #[cfg(not(feature = "double"))]
                    params.push(Value::F32(*v));
                }
                NodeTerminalRole::Vec2(v) => {
                    #[cfg(feature = "double")]
                    {
                        params.push(Value::F64(v.x));
                        params.push(Value::F64(v.y));
                    }
                    #[cfg(not(feature = "double"))]
                    {
                        params.push(Value::F32(v.x));
                        params.push(Value::F32(v.y));
                    }
                }
                NodeTerminalRole::Vec3(v) => {
                    #[cfg(feature = "double")]
                    {
                        params.push(Value::F64(v.x));
                        params.push(Value::F64(v.y));
                        params.push(Value::F64(v.z));
                    }
                    #[cfg(not(feature = "double"))]
                    {
                        params.push(Value::F32(v.x));
                        params.push(Value::F32(v.y));
                        params.push(Value::F32(v.z));
                    }
                }
                NodeTerminalRole::Vec4(v) => {
                    #[cfg(feature = "double")]
                    {
                        params.push(Value::F64(v.x));
                        params.push(Value::F64(v.y));
                        params.push(Value::F64(v.z));
                        params.push(Value::F64(v.w));
                    }
                    #[cfg(not(feature = "double"))]
                    {
                        params.push(Value::F32(v.x));
                        params.push(Value::F32(v.y));
                        params.push(Value::F32(v.z));
                        params.push(Value::F32(v.w));
                    }
                }
            }
        }
    }

    /// Get the output value of the given name.
    pub fn get_output_value(&self, name: &str) -> Option<NodeTerminalRole> {
        let mut offset = 0;
        for terminal in &self.outputs {
            if terminal.name == name {
                let slice = &self.output;
                let role = &terminal.role;
                return Some(match role {
                    NodeTerminalRole::Vec1(_) => NodeTerminalRole::Vec1(slice[offset]),
                    NodeTerminalRole::Vec2(_) => {
                        NodeTerminalRole::Vec2(Vec2::new(slice[offset], slice[offset + 1]))
                    }
                    NodeTerminalRole::Vec3(_) => NodeTerminalRole::Vec3(Vec3::new(
                        slice[offset],
                        slice[offset + 1],
                        slice[offset + 2],
                    )),
                    NodeTerminalRole::Vec4(_) => NodeTerminalRole::Vec4(Vec4::new(
                        slice[offset],
                        slice[offset + 1],
                        slice[offset + 2],
                        slice[offset + 3],
                    )),
                });
            }
            offset += terminal.role.len();
        }
        None
    }

    /// Parse and read the Inputs: and Outputs: fields from the node source.
    fn parse_terminals(&self, source: &str) -> (Vec<NodeTerminal>, Vec<NodeTerminal>) {
        use crate::utils::*;

        let mut inputs = vec![];
        let mut outputs = vec![];

        let re_terminal =
            Regex::new(r"(?m)^\s*(\w+):\s*(\w+)(?:\s*=\s*(.+))?\s*$").expect("Invalid regex");

        let mut current_section = None;
        let mut offset_input = 0;
        let mut offset_output = 0;

        for line in source.lines() {
            let line = line.trim();

            if line.eq_ignore_ascii_case("Inputs:") {
                current_section = Some("input");
                continue;
            } else if line.eq_ignore_ascii_case("Outputs:") {
                current_section = Some("output");
                continue;
            }

            if let Some(cap) = re_terminal.captures(line) {
                let name = cap[1].to_string();
                let ty = cap[2].to_string();
                let default = cap.get(3).map(|m| m.as_str().trim().to_string());

                let role = match (ty.as_str(), default) {
                    ("float", Some(d)) => NodeTerminalRole::Vec1(parse_float(&d)),
                    ("vec2", Some(d)) => NodeTerminalRole::Vec2(parse_vec2(&d)),
                    ("vec3", Some(d)) => NodeTerminalRole::Vec3(parse_vec3(&d)),
                    ("vec4", Some(d)) => NodeTerminalRole::Vec4(parse_vec4(&d)),
                    ("float", None) => NodeTerminalRole::Vec1(0.0),
                    ("vec2", None) => NodeTerminalRole::Vec2(Vec2::zero()),
                    ("vec3", None) => NodeTerminalRole::Vec3(Vec3::zero()),
                    ("vec4", None) => NodeTerminalRole::Vec4(Vec4::zero()),
                    _ => continue,
                };

                if current_section == Some("input") {
                    let t = NodeTerminal::with_swizzle_offset(name, role, offset_input);
                    offset_input += t.role.len();
                    inputs.push(t);
                } else if current_section == Some("output") {
                    let t = NodeTerminal::with_swizzle_offset(name, role, offset_output);
                    offset_output += t.role.len();
                    outputs.push(t);
                }
            }
        }

        (inputs, outputs)
    }
}
