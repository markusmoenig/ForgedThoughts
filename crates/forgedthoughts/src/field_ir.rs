use std::collections::HashMap;

use crate::ast::{BinaryOp, Expr, MaterialFunctionStatement, UnaryOp};
use crate::jit::{JitFunction, compile_jit_function};
use crate::vm::{VmFunction, VmInstruction};

#[derive(Debug, Clone, PartialEq)]
pub struct FieldIrProgram {
    pub expr: FieldIrExpr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FieldIrExpr {
    Number(f32),
    X,
    Z,
    Unary {
        op: UnaryOp,
        expr: Box<FieldIrExpr>,
    },
    Binary {
        lhs: Box<FieldIrExpr>,
        op: BinaryOp,
        rhs: Box<FieldIrExpr>,
    },
    Call {
        name: String,
        args: Vec<FieldIrExpr>,
    },
}

#[allow(dead_code)]
pub fn lower_expr_to_field_ir(expr: &Expr) -> Option<FieldIrExpr> {
    lower_expr_to_field_ir_with_env(expr, &HashMap::new())
}

fn lower_expr_to_field_ir_with_env(
    expr: &Expr,
    env: &HashMap<String, FieldIrExpr>,
) -> Option<FieldIrExpr> {
    match expr {
        Expr::Number(v) => Some(FieldIrExpr::Number(*v as f32)),
        Expr::Ident(name) if name == "x" => Some(FieldIrExpr::X),
        Expr::Ident(name) if name == "z" => Some(FieldIrExpr::Z),
        Expr::Ident(name) => env.get(name).cloned(),
        Expr::Unary { op, expr } => Some(FieldIrExpr::Unary {
            op: *op,
            expr: Box::new(lower_expr_to_field_ir_with_env(expr, env)?),
        }),
        Expr::Binary { lhs, op, rhs } => Some(FieldIrExpr::Binary {
            lhs: Box::new(lower_expr_to_field_ir_with_env(lhs, env)?),
            op: *op,
            rhs: Box::new(lower_expr_to_field_ir_with_env(rhs, env)?),
        }),
        Expr::Call { callee, args } => {
            let Expr::Ident(name) = callee.as_ref() else {
                return None;
            };
            if !matches!(
                name.as_str(),
                "clamp"
                    | "sqrt"
                    | "min"
                    | "max"
                    | "mix"
                    | "abs"
                    | "floor"
                    | "ceil"
                    | "fract"
                    | "pow"
                    | "sin"
                    | "cos"
                    | "smoothstep"
                    | "saturate"
                    | "step"
            ) {
                return None;
            }
            Some(FieldIrExpr::Call {
                name: name.clone(),
                args: args
                    .iter()
                    .map(|arg| lower_expr_to_field_ir_with_env(arg, env))
                    .collect::<Option<Vec<_>>>()?,
            })
        }
        _ => None,
    }
}

pub fn lower_body_to_field_ir(body: &[MaterialFunctionStatement]) -> Option<FieldIrProgram> {
    let mut env = HashMap::new();
    for stmt in body {
        match stmt {
            MaterialFunctionStatement::Binding { name, expr } => {
                let lowered = lower_expr_to_field_ir_with_env(expr, &env)?;
                env.insert(name.clone(), lowered);
            }
            MaterialFunctionStatement::Return { expr } => {
                return Some(FieldIrProgram {
                    expr: lower_expr_to_field_ir_with_env(expr, &env)?,
                });
            }
            MaterialFunctionStatement::ForLoop { .. } => return None,
        }
    }
    None
}

pub fn compile_field_ir_program(name: &str, program: &FieldIrProgram) -> Option<JitFunction> {
    let vm = compile_field_ir_to_vm(program);
    compile_jit_function(name, &vm)
}

pub fn compile_field_ir_to_vm(program: &FieldIrProgram) -> VmFunction {
    let simplified = simplify_field_ir_program(program);
    let mut code = Vec::new();
    compile_field_ir_expr_to_vm(&simplified.expr, &mut code);
    code.push(VmInstruction::Return);
    VmFunction {
        params: vec!["x".to_string(), "z".to_string()],
        code,
    }
}

pub fn simplify_field_ir_program(program: &FieldIrProgram) -> FieldIrProgram {
    FieldIrProgram {
        expr: simplify_field_ir_expr(&program.expr),
    }
}

fn compile_field_ir_expr_to_vm(expr: &FieldIrExpr, code: &mut Vec<VmInstruction>) {
    match expr {
        FieldIrExpr::Number(v) => code.push(VmInstruction::PushNumber(*v)),
        FieldIrExpr::X => code.push(VmInstruction::LoadName("x".to_string())),
        FieldIrExpr::Z => code.push(VmInstruction::LoadName("z".to_string())),
        FieldIrExpr::Unary { op, expr } => {
            compile_field_ir_expr_to_vm(expr, code);
            code.push(VmInstruction::Unary(*op));
        }
        FieldIrExpr::Binary { lhs, op, rhs } => {
            compile_field_ir_expr_to_vm(lhs, code);
            compile_field_ir_expr_to_vm(rhs, code);
            code.push(VmInstruction::Binary(*op));
        }
        FieldIrExpr::Call { name, args } => {
            for arg in args {
                compile_field_ir_expr_to_vm(arg, code);
            }
            code.push(VmInstruction::CallNamed {
                name: name.clone(),
                argc: args.len(),
            });
        }
    }
}

fn simplify_field_ir_expr(expr: &FieldIrExpr) -> FieldIrExpr {
    match expr {
        FieldIrExpr::Number(_) | FieldIrExpr::X | FieldIrExpr::Z => expr.clone(),
        FieldIrExpr::Unary { op, expr } => {
            let expr = simplify_field_ir_expr(expr);
            if let Some(value) = eval_unary_const(*op, &expr) {
                return FieldIrExpr::Number(value);
            }
            FieldIrExpr::Unary {
                op: *op,
                expr: Box::new(expr),
            }
        }
        FieldIrExpr::Binary { lhs, op, rhs } => {
            let lhs = simplify_field_ir_expr(lhs);
            let rhs = simplify_field_ir_expr(rhs);
            if let Some(value) = eval_binary_const(&lhs, *op, &rhs) {
                return FieldIrExpr::Number(value);
            }
            simplify_binary_expr(lhs, *op, rhs)
        }
        FieldIrExpr::Call { name, args } => {
            let args = args.iter().map(simplify_field_ir_expr).collect::<Vec<_>>();
            if let Some(value) = eval_call_const(name, &args) {
                return FieldIrExpr::Number(value);
            }
            FieldIrExpr::Call {
                name: name.clone(),
                args,
            }
        }
    }
}

fn simplify_binary_expr(lhs: FieldIrExpr, op: BinaryOp, rhs: FieldIrExpr) -> FieldIrExpr {
    match (&lhs, op, &rhs) {
        (FieldIrExpr::Number(0.0), BinaryOp::Add, _) => rhs,
        (_, BinaryOp::Add, FieldIrExpr::Number(0.0)) => lhs,
        (_, BinaryOp::Sub, FieldIrExpr::Number(0.0)) => lhs,
        (_, BinaryOp::Mul, FieldIrExpr::Number(1.0)) => lhs,
        (FieldIrExpr::Number(1.0), BinaryOp::Mul, _) => rhs,
        (_, BinaryOp::Mul, FieldIrExpr::Number(0.0)) => FieldIrExpr::Number(0.0),
        (FieldIrExpr::Number(0.0), BinaryOp::Mul, _) => FieldIrExpr::Number(0.0),
        (_, BinaryOp::Div, FieldIrExpr::Number(1.0)) => lhs,
        _ => FieldIrExpr::Binary {
            lhs: Box::new(lhs),
            op,
            rhs: Box::new(rhs),
        },
    }
}

fn eval_unary_const(op: UnaryOp, expr: &FieldIrExpr) -> Option<f32> {
    let FieldIrExpr::Number(value) = expr else {
        return None;
    };
    Some(match op {
        UnaryOp::Neg => -*value,
    })
}

fn eval_binary_const(lhs: &FieldIrExpr, op: BinaryOp, rhs: &FieldIrExpr) -> Option<f32> {
    let (FieldIrExpr::Number(lhs), FieldIrExpr::Number(rhs)) = (lhs, rhs) else {
        return None;
    };
    Some(match op {
        BinaryOp::Add => lhs + rhs,
        BinaryOp::Sub => lhs - rhs,
        BinaryOp::Mul => lhs * rhs,
        BinaryOp::Div => lhs / rhs,
        BinaryOp::Intersect => return None,
    })
}

fn eval_call_const(name: &str, args: &[FieldIrExpr]) -> Option<f32> {
    let args = args
        .iter()
        .map(|arg| match arg {
            FieldIrExpr::Number(v) => Some(*v),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    Some(match (name, args.as_slice()) {
        ("abs", [x]) => x.abs(),
        ("floor", [x]) => x.floor(),
        ("ceil", [x]) => x.ceil(),
        ("fract", [x]) => x - x.floor(),
        ("sqrt", [x]) => x.sqrt(),
        ("sin", [x]) => x.sin(),
        ("cos", [x]) => x.cos(),
        ("saturate", [x]) => x.clamp(0.0, 1.0),
        ("min", [a, b]) => a.min(*b),
        ("max", [a, b]) => a.max(*b),
        ("pow", [x, y]) => x.powf(*y),
        ("step", [edge, x]) => {
            if x < edge {
                0.0
            } else {
                1.0
            }
        }
        ("clamp", [x, a, b]) => x.clamp(a.min(*b), a.max(*b)),
        ("mix", [x, y, a]) => x + (y - x) * a,
        ("smoothstep", [edge0, edge1, x]) => {
            let span = edge1 - edge0;
            let t = if span.abs() <= f32::EPSILON {
                if x < edge0 { 0.0 } else { 1.0 }
            } else {
                ((x - edge0) / span).clamp(0.0, 1.0)
            };
            t * t * (3.0 - 2.0 * t)
        }
        _ => return None,
    })
}
