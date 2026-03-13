use thiserror::Error;

use crate::ast::{
    BinaryOp, EnvironmentDef, Expr, FunctionDef, MaterialDef, MaterialFunctionStatement,
    MaterialStatement, Program, SdfDef, SdfStatement, Statement, UnaryOp,
};
use crate::lexer::{LexError, Token, TokenKind, tokenize};

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("{0}")]
    Lex(#[from] LexError),
    #[error("expected {expected} at byte {offset}")]
    Expected {
        expected: &'static str,
        offset: usize,
    },
    #[error("unexpected token at byte {offset}")]
    UnexpectedToken { offset: usize },
    #[error("unexpected end of input")]
    UnexpectedEof,
}

pub fn parse_program(source: &str) -> Result<Program, ParseError> {
    let tokens = tokenize(source)?;
    let mut parser = Parser { tokens, pos: 0 };
    parser.parse_program()
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn parse_program(&mut self) -> Result<Program, ParseError> {
        let mut statements = Vec::new();
        while !self.is_eof() {
            statements.push(self.parse_statement()?);
        }
        Ok(Program { statements })
    }

    fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        if self.matches_kind(TokenKind::Import) {
            return self.parse_import();
        }
        if self.matches_kind(TokenKind::Export) {
            return self.parse_export();
        }

        if self.matches_ident_literal("material") {
            return self.parse_material_def();
        }

        if self.matches_ident_literal("sdf") {
            return self.parse_sdf_def();
        }

        if self.matches_ident_literal("environment") {
            return self.parse_environment_def();
        }

        if self.matches_kind(TokenKind::Fn) {
            return self.parse_function_def();
        }

        if self.matches_kind(TokenKind::Let) {
            return self.parse_binding(false);
        }

        if self.matches_kind(TokenKind::Var) {
            return self.parse_binding(true);
        }

        self.parse_assignment()
    }

    fn parse_import(&mut self) -> Result<Statement, ParseError> {
        let path = self.expect_string()?;
        let alias = if self.matches_ident_literal("as") {
            Some(self.expect_ident()?)
        } else {
            None
        };
        self.expect_kind(TokenKind::Semicolon, ";")?;
        Ok(Statement::Import { path, alias })
    }

    fn parse_export(&mut self) -> Result<Statement, ParseError> {
        self.expect_kind(TokenKind::LBrace, "{")?;
        let mut names = Vec::new();
        if !self.matches_kind(TokenKind::RBrace) {
            loop {
                names.push(self.expect_ident()?);
                if self.matches_kind(TokenKind::Comma) {
                    continue;
                }
                self.expect_kind(TokenKind::RBrace, "}")?;
                break;
            }
        }
        self.expect_kind(TokenKind::Semicolon, ";")?;
        Ok(Statement::Export(names))
    }

    fn parse_function_def(&mut self) -> Result<Statement, ParseError> {
        let name = self.expect_ident()?;
        let params = self.parse_function_params()?;
        let body = if self.matches_kind(TokenKind::Equal) {
            let expr = self.parse_expr()?;
            self.expect_kind(TokenKind::Semicolon, ";")?;
            vec![MaterialFunctionStatement::Return { expr }]
        } else {
            self.parse_material_function_body()?
        };
        Ok(Statement::FunctionDef(FunctionDef { name, params, body }))
    }

    fn parse_material_def(&mut self) -> Result<Statement, ParseError> {
        let name = self.expect_ident()?;
        self.expect_kind(TokenKind::LBrace, "{")?;
        let mut model = None;
        let mut metadata = Vec::new();
        let mut statements = Vec::new();

        while !self.matches_kind(TokenKind::RBrace) {
            if self.matches_kind(TokenKind::Let) {
                let binding_name = self.expect_ident()?;
                self.expect_kind(TokenKind::Equal, "=")?;
                let expr = self.parse_expr()?;
                self.expect_kind(TokenKind::Semicolon, ";")?;
                statements.push(MaterialStatement::Binding {
                    name: binding_name,
                    expr,
                });
                continue;
            }

            if self.matches_kind(TokenKind::Fn) {
                let fn_name = self.expect_ident()?;
                let params = self.parse_function_params()?;
                let body = if self.matches_kind(TokenKind::Equal) {
                    let expr = self.parse_expr()?;
                    self.expect_kind(TokenKind::Semicolon, ";")?;
                    vec![MaterialFunctionStatement::Return { expr }]
                } else {
                    self.parse_material_function_body()?
                };
                statements.push(MaterialStatement::Function {
                    name: fn_name,
                    params,
                    body,
                });
                continue;
            }

            let field = self.expect_ident()?;
            if self.matches_kind(TokenKind::Equal) {
                let expr = self.parse_expr()?;
                self.expect_kind(TokenKind::Semicolon, ";")?;
                statements.push(MaterialStatement::Property { name: field, expr });
                continue;
            }
            if field == "model" {
                self.expect_kind(TokenKind::Colon, ":")?;
                model = Some(self.expect_ident()?);
                self.expect_kind(TokenKind::Semicolon, ";")?;
                continue;
            }
            if matches!(field.as_str(), "name" | "description" | "tags" | "params") {
                self.expect_kind(TokenKind::Colon, ":")?;
                let expr = self.parse_expr()?;
                self.expect_kind(TokenKind::Semicolon, ";")?;
                metadata.push((field, expr));
                continue;
            }
            return Err(ParseError::Expected {
                expected: "let, fn, property assignment, or model",
                offset: self.current_offset(),
            });
        }

        self.expect_kind(TokenKind::Semicolon, ";")?;
        Ok(Statement::MaterialDef(MaterialDef {
            name,
            model: model.unwrap_or_else(|| "Standard".to_string()),
            metadata,
            statements,
        }))
    }

    fn parse_sdf_def(&mut self) -> Result<Statement, ParseError> {
        let name = self.expect_ident()?;
        self.expect_kind(TokenKind::LBrace, "{")?;
        let mut metadata = Vec::new();
        let mut statements = Vec::new();

        while !self.matches_kind(TokenKind::RBrace) {
            if self.matches_kind(TokenKind::Let) {
                let binding_name = self.expect_ident()?;
                self.expect_kind(TokenKind::Equal, "=")?;
                let expr = self.parse_expr()?;
                self.expect_kind(TokenKind::Semicolon, ";")?;
                statements.push(SdfStatement::Binding {
                    name: binding_name,
                    expr,
                });
                continue;
            }

            if self.matches_kind(TokenKind::Fn) {
                let fn_name = self.expect_ident()?;
                let params = self.parse_function_params()?;
                let body = if self.matches_kind(TokenKind::Equal) {
                    let expr = self.parse_expr()?;
                    self.expect_kind(TokenKind::Semicolon, ";")?;
                    vec![MaterialFunctionStatement::Return { expr }]
                } else {
                    self.parse_material_function_body()?
                };
                statements.push(SdfStatement::Function {
                    name: fn_name,
                    params,
                    body,
                });
                continue;
            }

            let field = self.expect_ident()?;
            if matches!(field.as_str(), "name" | "description" | "tags" | "params") {
                self.expect_kind(TokenKind::Colon, ":")?;
                let expr = self.parse_expr()?;
                self.expect_kind(TokenKind::Semicolon, ";")?;
                metadata.push((field, expr));
                continue;
            }

            return Err(ParseError::Expected {
                expected: "let, fn, or metadata field",
                offset: self.current_offset(),
            });
        }

        self.expect_kind(TokenKind::Semicolon, ";")?;
        Ok(Statement::SdfDef(SdfDef {
            name,
            metadata,
            statements,
        }))
    }

    fn parse_environment_def(&mut self) -> Result<Statement, ParseError> {
        let name = self.expect_ident()?;
        self.expect_kind(TokenKind::LBrace, "{")?;
        let mut metadata = Vec::new();
        let mut statements = Vec::new();

        while !self.matches_kind(TokenKind::RBrace) {
            if self.matches_kind(TokenKind::Let) {
                let binding_name = self.expect_ident()?;
                self.expect_kind(TokenKind::Equal, "=")?;
                let expr = self.parse_expr()?;
                self.expect_kind(TokenKind::Semicolon, ";")?;
                statements.push(MaterialStatement::Binding {
                    name: binding_name,
                    expr,
                });
                continue;
            }

            if self.matches_kind(TokenKind::Fn) {
                let fn_name = self.expect_ident()?;
                let params = self.parse_function_params()?;
                let body = if self.matches_kind(TokenKind::Equal) {
                    let expr = self.parse_expr()?;
                    self.expect_kind(TokenKind::Semicolon, ";")?;
                    vec![MaterialFunctionStatement::Return { expr }]
                } else {
                    self.parse_material_function_body()?
                };
                statements.push(MaterialStatement::Function {
                    name: fn_name,
                    params,
                    body,
                });
                continue;
            }

            let field = self.expect_ident()?;
            if matches!(field.as_str(), "name" | "description" | "tags" | "params") {
                self.expect_kind(TokenKind::Colon, ":")?;
                let expr = self.parse_expr()?;
                self.expect_kind(TokenKind::Semicolon, ";")?;
                metadata.push((field, expr));
                continue;
            }

            return Err(ParseError::Expected {
                expected: "let, fn, or metadata field",
                offset: self.current_offset(),
            });
        }

        self.expect_kind(TokenKind::Semicolon, ";")?;
        Ok(Statement::EnvironmentDef(EnvironmentDef {
            name,
            metadata,
            statements,
        }))
    }

    fn parse_material_function_body(
        &mut self,
    ) -> Result<Vec<MaterialFunctionStatement>, ParseError> {
        self.expect_kind(TokenKind::LBrace, "{")?;
        let mut body = Vec::new();
        while !self.matches_kind(TokenKind::RBrace) {
            if self.matches_kind(TokenKind::Let) {
                let name = self.expect_ident()?;
                self.expect_kind(TokenKind::Equal, "=")?;
                let expr = self.parse_expr()?;
                self.expect_kind(TokenKind::Semicolon, ";")?;
                body.push(MaterialFunctionStatement::Binding { name, expr });
                continue;
            }
            if self.matches_kind(TokenKind::Return) {
                let expr = self.parse_expr()?;
                self.expect_kind(TokenKind::Semicolon, ";")?;
                body.push(MaterialFunctionStatement::Return { expr });
                continue;
            }
            return Err(ParseError::Expected {
                expected: "let or return",
                offset: self.current_offset(),
            });
        }
        Ok(body)
    }

    fn parse_function_params(&mut self) -> Result<Vec<String>, ParseError> {
        self.expect_kind(TokenKind::LParen, "(")?;
        let mut params = Vec::new();
        if !self.matches_kind(TokenKind::RParen) {
            loop {
                params.push(self.expect_ident()?);
                if self.matches_kind(TokenKind::Comma) {
                    continue;
                }
                self.expect_kind(TokenKind::RParen, ")")?;
                break;
            }
        }
        Ok(params)
    }

    fn parse_binding(&mut self, mutable: bool) -> Result<Statement, ParseError> {
        let name = self.expect_ident()?;
        self.expect_kind(TokenKind::Equal, "=")?;
        let expr = self.parse_expr()?;
        self.expect_kind(TokenKind::Semicolon, ";")?;
        Ok(Statement::Binding {
            name,
            mutable,
            expr,
        })
    }

    fn parse_assignment(&mut self) -> Result<Statement, ParseError> {
        let mut path = vec![self.expect_ident()?];
        while self.matches_kind(TokenKind::Dot) {
            path.push(self.expect_ident()?);
        }
        self.expect_kind(TokenKind::Equal, "=")?;
        let expr = self.parse_expr()?;
        self.expect_kind(TokenKind::Semicolon, ";")?;
        Ok(Statement::Assign { path, expr })
    }

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_add_sub()
    }

    fn parse_add_sub(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_mul_div()?;
        loop {
            let op = if self.matches_kind(TokenKind::Plus) {
                Some(BinaryOp::Add)
            } else if self.matches_kind(TokenKind::Minus) {
                Some(BinaryOp::Sub)
            } else if self.matches_kind(TokenKind::Amp) {
                Some(BinaryOp::Intersect)
            } else {
                None
            };

            if let Some(op) = op {
                let rhs = self.parse_mul_div()?;
                expr = Expr::Binary {
                    lhs: Box::new(expr),
                    op,
                    rhs: Box::new(rhs),
                };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_mul_div(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_unary()?;
        loop {
            let op = if self.matches_kind(TokenKind::Star) {
                Some(BinaryOp::Mul)
            } else if self.matches_kind(TokenKind::Slash) {
                Some(BinaryOp::Div)
            } else {
                None
            };

            if let Some(op) = op {
                let rhs = self.parse_unary()?;
                expr = Expr::Binary {
                    lhs: Box::new(expr),
                    op,
                    rhs: Box::new(rhs),
                };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        if self.matches_kind(TokenKind::Minus) {
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Neg,
                expr: Box::new(expr),
            });
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_primary()?;
        loop {
            if self.matches_kind(TokenKind::Dot) {
                let field = self.expect_ident()?;
                expr = Expr::Member {
                    target: Box::new(expr),
                    field,
                };
                continue;
            }

            if self.matches_kind(TokenKind::LParen) {
                let mut args = Vec::new();
                if !self.matches_kind(TokenKind::RParen) {
                    loop {
                        args.push(self.parse_expr()?);
                        if self.matches_kind(TokenKind::Comma) {
                            continue;
                        }
                        self.expect_kind(TokenKind::RParen, ")")?;
                        break;
                    }
                }
                expr = Expr::Call {
                    callee: Box::new(expr),
                    args,
                };
                continue;
            }

            break;
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        if self.matches_kind(TokenKind::Fn) {
            let params = self.parse_function_params()?;
            let body = if self.matches_kind(TokenKind::Equal) {
                let expr = self.parse_expr()?;
                self.expect_kind(TokenKind::Semicolon, ";")?;
                vec![MaterialFunctionStatement::Return { expr }]
            } else {
                self.parse_material_function_body()?
            };
            return Ok(Expr::FunctionLiteral { params, body });
        }
        if self.matches_kind(TokenKind::LParen) {
            let expr = self.parse_expr()?;
            self.expect_kind(TokenKind::RParen, ")")?;
            return Ok(expr);
        }
        if self.matches_kind(TokenKind::LBrace) {
            let fields = self.parse_object_fields()?;
            return Ok(Expr::ObjectLiteral {
                type_name: "anonymous".to_string(),
                fields,
            });
        }
        if self.matches_kind(TokenKind::LBracket) {
            let mut items = Vec::new();
            if !self.matches_kind(TokenKind::RBracket) {
                loop {
                    items.push(self.parse_expr()?);
                    if self.matches_kind(TokenKind::Comma) {
                        continue;
                    }
                    self.expect_kind(TokenKind::RBracket, "]")?;
                    break;
                }
            }
            return Ok(Expr::Array(items));
        }

        match self.peek_kind() {
            Some(TokenKind::String(value)) => {
                let value = value.clone();
                self.pos += 1;
                Ok(Expr::String(value))
            }
            Some(TokenKind::HexColor(hex)) => {
                let hex = hex.clone();
                self.pos += 1;
                Ok(hex_color_expr(&hex)?)
            }
            Some(TokenKind::Number(value)) => {
                let value = *value;
                self.pos += 1;
                Ok(Expr::Number(value))
            }
            Some(TokenKind::Ident(_)) => {
                let segments = self.parse_ident_chain()?;
                if self.matches_kind(TokenKind::LBrace) {
                    let fields = self.parse_object_fields()?;
                    Ok(Expr::ObjectLiteral {
                        type_name: segments.join("."),
                        fields,
                    })
                } else {
                    let mut expr = Expr::Ident(segments[0].clone());
                    for field in segments.iter().skip(1) {
                        expr = Expr::Member {
                            target: Box::new(expr),
                            field: field.clone(),
                        };
                    }
                    Ok(expr)
                }
            }
            Some(_) => Err(ParseError::UnexpectedToken {
                offset: self.current_offset(),
            }),
            None => Err(ParseError::UnexpectedEof),
        }
    }

    fn parse_object_fields(&mut self) -> Result<Vec<(String, Expr)>, ParseError> {
        let mut fields = Vec::new();
        if self.matches_kind(TokenKind::RBrace) {
            return Ok(fields);
        }

        loop {
            let key = self.expect_ident()?;
            self.expect_kind(TokenKind::Colon, ":")?;
            let value = self.parse_expr()?;
            fields.push((key, value));

            if self.matches_kind(TokenKind::Comma) {
                if self.matches_kind(TokenKind::RBrace) {
                    return Ok(fields);
                }
                continue;
            }

            self.expect_kind(TokenKind::RBrace, "}")?;
            break;
        }
        Ok(fields)
    }

    fn expect_ident(&mut self) -> Result<String, ParseError> {
        match self.peek_kind() {
            Some(TokenKind::Ident(name)) => {
                let name = name.clone();
                self.pos += 1;
                Ok(name)
            }
            Some(_) => Err(ParseError::Expected {
                expected: "identifier",
                offset: self.current_offset(),
            }),
            None => Err(ParseError::UnexpectedEof),
        }
    }

    fn expect_string(&mut self) -> Result<String, ParseError> {
        match self.peek_kind() {
            Some(TokenKind::String(value)) => {
                let value = value.clone();
                self.pos += 1;
                Ok(value)
            }
            Some(_) => Err(ParseError::Expected {
                expected: "string literal",
                offset: self.current_offset(),
            }),
            None => Err(ParseError::UnexpectedEof),
        }
    }

    fn parse_ident_chain(&mut self) -> Result<Vec<String>, ParseError> {
        let mut segments = vec![self.expect_ident()?];
        while self.matches_kind(TokenKind::Dot) {
            segments.push(self.expect_ident()?);
        }
        Ok(segments)
    }

    fn expect_kind(&mut self, kind: TokenKind, expected: &'static str) -> Result<(), ParseError> {
        if self.matches_kind(kind) {
            Ok(())
        } else {
            Err(ParseError::Expected {
                expected,
                offset: self.current_offset(),
            })
        }
    }

    fn matches_kind(&mut self, kind: TokenKind) -> bool {
        if let Some(current) = self.peek_kind()
            && std::mem::discriminant(current) == std::mem::discriminant(&kind)
        {
            self.pos += 1;
            return true;
        }
        false
    }

    fn peek_kind(&self) -> Option<&TokenKind> {
        self.tokens.get(self.pos).map(|t| &t.kind)
    }

    fn matches_ident_literal(&mut self, expected: &str) -> bool {
        match self.peek_kind() {
            Some(TokenKind::Ident(name)) if name == expected => {
                self.pos += 1;
                true
            }
            _ => false,
        }
    }

    fn current_offset(&self) -> usize {
        self.tokens
            .get(self.pos)
            .map_or_else(|| self.tokens.last().map_or(0, |t| t.start), |t| t.start)
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.tokens.len()
    }
}

fn hex_color_expr(hex: &str) -> Result<Expr, ParseError> {
    let [r, g, b] = parse_hex_color(hex).ok_or(ParseError::UnexpectedToken { offset: 0 })?;
    Ok(Expr::ObjectLiteral {
        type_name: "vec3".to_string(),
        fields: vec![
            ("x".to_string(), Expr::Number(r)),
            ("y".to_string(), Expr::Number(g)),
            ("z".to_string(), Expr::Number(b)),
        ],
    })
}

fn parse_hex_color(hex: &str) -> Option<[f64; 3]> {
    match hex.len() {
        3 => {
            let mut values = [0.0; 3];
            for (index, ch) in hex.chars().enumerate() {
                let value = ch.to_digit(16)? as u8;
                let expanded = (value << 4) | value;
                values[index] = f64::from(expanded) / 255.0;
            }
            Some(values)
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some([
                f64::from(r) / 255.0,
                f64::from(g) / 255.0,
                f64::from(b) / 255.0,
            ])
        }
        _ => None,
    }
}
