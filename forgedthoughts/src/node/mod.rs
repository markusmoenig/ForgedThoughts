use crate::{Scanner, TokenType};
use std::sync::Arc;
use wasmer::{Module, Store};

pub struct Node {
    pub name: String,

    source: String,
    pub wat: String,

    pub module: Option<Arc<Module>>,
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
            source: String::new(),
            wat: String::new(),
            module: None,
        }
    }

    pub fn compile(&mut self, source: String, high_precision: bool) -> Result<(), String> {
        let mut scanner = Scanner::new(source.clone());

        let token = scanner.scan_token(false);
        if token.kind != TokenType::Identifier && token.lexeme != "Node" {
            return Err(format!(
                "Expected 'Node' identifier at line {}.",
                token.line
            ));
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

        // Get RPU source

        if let Some((_, rpu)) = source.split_once("Source<RPU>") {
            let mut source = "export vec4 main(vec2 uv, vec2 resolution) {\n".to_string();
            source += rpu;
            source += "\n}";
            self.source = source;
            let rpu = rpu::RPU::new();
            match rpu.compile_to_wat(self.source.clone(), high_precision) {
                Ok(wat) => {
                    self.wat = wat;
                    let store = Store::default();
                    if let Ok(module) = Module::new(&store, &self.wat) {
                        self.module = Some(Arc::new(module));
                    }
                }
                Err(err) => return Err(format!("RPU: {}", err)),
            }
        } else {
            return Err("No 'Source<RPU> tag found.".into());
        }

        Ok(())
    }
}
