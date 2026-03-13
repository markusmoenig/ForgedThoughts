use crate::ast::{BinaryOp, Expr, FunctionDef, MaterialFunctionStatement, UnaryOp};

#[derive(Debug, Clone, PartialEq)]
pub enum VmInstruction {
    PushNumber(f32),
    PushString(String),
    LoadName(String),
    BuildArray(usize),
    BuildObject {
        type_name: String,
        field_names: Vec<String>,
    },
    LoadMember(String),
    Unary(UnaryOp),
    Binary(BinaryOp),
    CallNamed {
        name: String,
        argc: usize,
    },
    StoreLocal(String),
    Return,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VmFunction {
    pub params: Vec<String>,
    pub code: Vec<VmInstruction>,
}

pub fn compile_function(def: &FunctionDef) -> Option<VmFunction> {
    compile_function_parts(&def.params, &def.body)
}

pub fn compile_function_parts(
    params: &[String],
    body: &[MaterialFunctionStatement],
) -> Option<VmFunction> {
    let mut code = Vec::new();
    for stmt in body {
        compile_stmt(stmt, &mut code)?;
    }
    Some(VmFunction {
        params: params.to_vec(),
        code,
    })
}

fn compile_stmt(stmt: &MaterialFunctionStatement, code: &mut Vec<VmInstruction>) -> Option<()> {
    match stmt {
        MaterialFunctionStatement::Binding { name, expr } => {
            compile_expr(expr, code)?;
            code.push(VmInstruction::StoreLocal(name.clone()));
        }
        MaterialFunctionStatement::Return { expr } => {
            compile_expr(expr, code)?;
            code.push(VmInstruction::Return);
        }
    }
    Some(())
}

fn compile_expr(expr: &Expr, code: &mut Vec<VmInstruction>) -> Option<()> {
    match expr {
        Expr::Number(n) => code.push(VmInstruction::PushNumber(*n as f32)),
        Expr::String(value) => code.push(VmInstruction::PushString(value.clone())),
        Expr::Array(items) => {
            for item in items {
                compile_expr(item, code)?;
            }
            code.push(VmInstruction::BuildArray(items.len()));
        }
        Expr::Ident(name) => code.push(VmInstruction::LoadName(name.clone())),
        Expr::ObjectLiteral { type_name, fields } => {
            for (_, expr) in fields {
                compile_expr(expr, code)?;
            }
            code.push(VmInstruction::BuildObject {
                type_name: type_name.clone(),
                field_names: fields.iter().map(|(name, _)| name.clone()).collect(),
            });
        }
        Expr::Binary { lhs, op, rhs } => {
            compile_expr(lhs, code)?;
            compile_expr(rhs, code)?;
            code.push(VmInstruction::Binary(*op));
        }
        Expr::Member { target, field } => {
            compile_expr(target, code)?;
            code.push(VmInstruction::LoadMember(field.clone()));
        }
        Expr::Call { callee, args } => {
            if let Expr::Member { field, .. } = callee.as_ref()
                && is_unsupported_method_name(field)
            {
                return None;
            }
            let name = flatten_member_expr(callee)?;
            for arg in args {
                compile_expr(arg, code)?;
            }
            code.push(VmInstruction::CallNamed {
                name,
                argc: args.len(),
            });
        }
        Expr::Unary { op, expr } => {
            compile_expr(expr, code)?;
            code.push(VmInstruction::Unary(*op));
        }
        Expr::FunctionLiteral { .. } => return None,
    }
    Some(())
}

fn flatten_member_expr(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Ident(name) => Some(name.clone()),
        Expr::Member { target, field } => {
            Some(format!("{}.{}", flatten_member_expr(target)?, field))
        }
        _ => None,
    }
}

fn is_unsupported_method_name(name: &str) -> bool {
    matches!(
        name,
        "smooth"
            | "union_round"
            | "union_chamfer"
            | "union_columns"
            | "union_stairs"
            | "union_soft"
            | "intersect_round"
            | "intersect_chamfer"
            | "intersect_columns"
            | "intersect_stairs"
            | "diff_round"
            | "diff_chamfer"
            | "diff_columns"
            | "diff_stairs"
            | "pipe"
            | "engrave"
            | "groove"
            | "tongue"
            | "attach"
            | "align_x"
            | "align_y"
            | "align_z"
            | "right_of"
            | "left_of"
            | "on_top_of"
            | "below"
            | "in_front_of"
            | "behind"
            | "rotate_x"
            | "rotate_y"
            | "rotate_z"
            | "offset_x"
            | "offset_y"
            | "offset_z"
    )
}
