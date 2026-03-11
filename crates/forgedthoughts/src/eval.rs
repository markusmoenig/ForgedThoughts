use std::collections::HashMap;

use thiserror::Error;

use crate::ast::{
    BinaryOp, EnvironmentDef, Expr, FunctionDef, MaterialDef, MaterialFunctionStatement,
    MaterialStatement, Program, SdfDef, SdfFunctionStatement, SdfStatement, Statement, UnaryOp,
};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(f64),
    Object(ObjectValue),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectValue {
    pub type_name: Option<String>,
    pub fields: HashMap<String, Value>,
}

impl ObjectValue {
    fn empty() -> Self {
        Self {
            type_name: None,
            fields: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Binding {
    pub mutable: bool,
    pub value: Value,
}

#[derive(Debug, Clone)]
pub struct EvalState {
    pub bindings: HashMap<String, Binding>,
    pub function_defs: HashMap<String, FunctionDef>,
    pub material_defs: HashMap<String, MaterialDef>,
    pub sdf_defs: HashMap<String, SdfDef>,
    pub environment_defs: HashMap<String, EnvironmentDef>,
}

#[derive(Debug, Error)]
pub enum EvalError {
    #[error("undefined identifier '{0}'")]
    UndefinedIdentifier(String),
    #[error("cannot assign to immutable binding '{0}'")]
    ImmutableBinding(String),
    #[error("member access requires object value")]
    MemberAccessOnNonObject,
    #[error("call on unsupported callee")]
    UnsupportedCall,
    #[error("builtin vec3 expects exactly 1 or 3 numeric args")]
    InvalidVec3Call,
    #[error("builtin {name} expects exactly {expected} numeric args, got {got}")]
    InvalidBuiltinArity {
        name: &'static str,
        expected: usize,
        got: usize,
    },
    #[error("builtin {0} expects numeric args")]
    BuiltinNumericArgs(&'static str),
    #[error("builtin {0} expects number or vec3 args")]
    BuiltinNumericOrVec3Args(&'static str),
    #[error("builtin {0} expects vec3 args")]
    BuiltinVec3Args(&'static str),
    #[error("unary operation requires numeric or vec3 operand")]
    UnaryTypeMismatch,
    #[error("binary operation requires numeric operands")]
    BinaryTypeMismatch,
    #[error("material call depth exceeded")]
    MaterialCallDepthExceeded,
}

#[derive(Clone, Copy)]
struct MaterialRuntime<'a> {
    def: &'a MaterialDef,
    depth: usize,
}

#[derive(Clone, Copy)]
struct SdfRuntime<'a> {
    def: &'a SdfDef,
    depth: usize,
}

pub fn eval_program(program: &Program) -> Result<EvalState, EvalError> {
    let mut state = EvalState {
        bindings: HashMap::new(),
        function_defs: HashMap::new(),
        material_defs: HashMap::new(),
        sdf_defs: HashMap::new(),
        environment_defs: HashMap::new(),
    };

    for stmt in &program.statements {
        eval_statement(stmt, &mut state)?;
    }

    Ok(state)
}

fn eval_statement(stmt: &Statement, state: &mut EvalState) -> Result<(), EvalError> {
    match stmt {
        Statement::Import { .. } => Ok(()),
        Statement::Export(_) => Ok(()),
        Statement::Binding {
            name,
            mutable,
            expr,
        } => {
            let value = eval_expr(expr, state)?;
            state.bindings.insert(
                name.clone(),
                Binding {
                    mutable: *mutable,
                    value,
                },
            );
            Ok(())
        }
        Statement::Assign { path, expr } => {
            let value = eval_expr(expr, state)?;
            assign_path(path, value, state)
        }
        Statement::FunctionDef(def) => {
            state.function_defs.insert(def.name.clone(), def.clone());
            Ok(())
        }
        Statement::MaterialDef(def) => {
            state.material_defs.insert(def.name.clone(), def.clone());
            Ok(())
        }
        Statement::SdfDef(def) => {
            state.sdf_defs.insert(def.name.clone(), def.clone());
            Ok(())
        }
        Statement::EnvironmentDef(def) => {
            state.environment_defs.insert(def.name.clone(), def.clone());
            Ok(())
        }
    }
}

fn assign_path(path: &[String], value: Value, state: &mut EvalState) -> Result<(), EvalError> {
    let Some((binding_name, consumed)) = resolve_binding_path(&state.bindings, path) else {
        return Ok(());
    };
    let rest = &path[consumed..];

    let binding = state
        .bindings
        .get_mut(&binding_name)
        .ok_or_else(|| EvalError::UndefinedIdentifier(binding_name.clone()))?;

    if !binding.mutable {
        return Err(EvalError::ImmutableBinding(binding_name));
    }

    if rest.is_empty() {
        binding.value = value;
        return Ok(());
    }

    let obj = as_object_mut(&mut binding.value)?;
    assign_path_in_object(obj, rest, value)
}

fn assign_path_in_object(
    object: &mut ObjectValue,
    path: &[String],
    value: Value,
) -> Result<(), EvalError> {
    let Some((head, tail)) = path.split_first() else {
        return Ok(());
    };

    if tail.is_empty() {
        object.fields.insert(head.clone(), value);
        return Ok(());
    }

    let next = object
        .fields
        .entry(head.clone())
        .or_insert_with(|| Value::Object(ObjectValue::empty()));

    let nested_obj = as_object_mut(next)?;
    assign_path_in_object(nested_obj, tail, value)
}

fn eval_expr(expr: &Expr, state: &EvalState) -> Result<Value, EvalError> {
    eval_expr_in_material_scope(expr, state, &HashMap::new(), None, 0)
}

fn eval_expr_in_material_scope(
    expr: &Expr,
    state: &EvalState,
    locals: &HashMap<String, Value>,
    material_runtime: Option<MaterialRuntime<'_>>,
    top_level_depth: usize,
) -> Result<Value, EvalError> {
    match expr {
        Expr::Number(n) => Ok(Value::Number(*n)),
        Expr::Ident(name) => {
            if let Some(value) = locals.get(name) {
                return Ok(value.clone());
            }
            let binding = state
                .bindings
                .get(name)
                .ok_or_else(|| EvalError::UndefinedIdentifier(name.clone()))?;
            Ok(binding.value.clone())
        }
        Expr::ObjectLiteral { type_name, fields } => {
            let mut resolved_fields = HashMap::new();
            for (name, field_expr) in fields {
                resolved_fields.insert(
                    name.clone(),
                    eval_expr_in_material_scope(
                        field_expr,
                        state,
                        locals,
                        material_runtime,
                        top_level_depth,
                    )?,
                );
            }
            Ok(Value::Object(ObjectValue {
                type_name: Some(type_name.clone()),
                fields: resolved_fields,
            }))
        }
        Expr::Binary { lhs, op, rhs } => {
            let left =
                eval_expr_in_material_scope(lhs, state, locals, material_runtime, top_level_depth)?;
            let right =
                eval_expr_in_material_scope(rhs, state, locals, material_runtime, top_level_depth)?;
            eval_binary(left, *op, right)
        }
        Expr::Member { target, field } => {
            if let Some(name) = flatten_member_expr(expr)
                && let Some(binding) = state.bindings.get(&name)
            {
                return Ok(binding.value.clone());
            }
            let base = eval_expr_in_material_scope(
                target,
                state,
                locals,
                material_runtime,
                top_level_depth,
            )?;
            let obj = as_object(&base)?;
            obj.fields
                .get(field)
                .cloned()
                .ok_or(EvalError::UndefinedIdentifier(field.clone()))
        }
        Expr::Call { callee, args } => eval_call(
            callee,
            args,
            state,
            locals,
            material_runtime,
            top_level_depth,
        ),
        Expr::Unary { op, expr } => {
            let value = eval_expr_in_material_scope(
                expr,
                state,
                locals,
                material_runtime,
                top_level_depth,
            )?;
            eval_unary(*op, value)
        }
    }
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

fn resolve_binding_path(
    bindings: &HashMap<String, Binding>,
    path: &[String],
) -> Option<(String, usize)> {
    for len in (1..=path.len()).rev() {
        let candidate = path[..len].join(".");
        if bindings.contains_key(&candidate) {
            return Some((candidate, len));
        }
    }
    path.first().map(|root| (root.clone(), 1))
}

fn eval_binary(lhs: Value, op: BinaryOp, rhs: Value) -> Result<Value, EvalError> {
    if let (Value::Number(left), Value::Number(right)) = (&lhs, &rhs) {
        let out = match op {
            BinaryOp::Add => left + right,
            BinaryOp::Sub => left - right,
            BinaryOp::Intersect => return Err(EvalError::BinaryTypeMismatch),
            BinaryOp::Mul => left * right,
            BinaryOp::Div => left / right,
        };
        return Ok(Value::Number(out));
    }

    if matches!(
        op,
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div
    ) && let (Some(left), Some(right)) =
        (as_broadcastable_vec3(&lhs), as_broadcastable_vec3(&rhs))
    {
        let out = match op {
            BinaryOp::Add => [left[0] + right[0], left[1] + right[1], left[2] + right[2]],
            BinaryOp::Sub => [left[0] - right[0], left[1] - right[1], left[2] - right[2]],
            BinaryOp::Mul => [left[0] * right[0], left[1] * right[1], left[2] * right[2]],
            BinaryOp::Div => [left[0] / right[0], left[1] / right[1], left[2] / right[2]],
            BinaryOp::Intersect => return Err(EvalError::BinaryTypeMismatch),
        };
        return Ok(vec3_value(out));
    }

    match op {
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Intersect => {
            let op_name = match op {
                BinaryOp::Add => "add",
                BinaryOp::Sub => "sub",
                BinaryOp::Intersect => "intersect",
                BinaryOp::Mul | BinaryOp::Div => unreachable!(),
            };
            let mut fields = HashMap::new();
            fields.insert("lhs".to_string(), lhs);
            fields.insert("rhs".to_string(), rhs);
            Ok(Value::Object(ObjectValue {
                type_name: Some(op_name.to_string()),
                fields,
            }))
        }
        BinaryOp::Mul | BinaryOp::Div => Err(EvalError::BinaryTypeMismatch),
    }
}

fn build_sdf_member_value(
    field: &str,
    base: Value,
    args: Vec<Value>,
) -> Result<Option<Value>, EvalError> {
    let (type_name, fields): (&str, Vec<(&str, Value)>) = match field {
        "smooth" => {
            if args.len() != 1 {
                return Err(EvalError::UnsupportedCall);
            }
            let Value::Number(k) = args[0] else {
                return Err(EvalError::UnsupportedCall);
            };
            ("smooth", vec![("base", base), ("k", Value::Number(k))])
        }
        "round" | "bevel" | "chamfer" => {
            if args.len() != 1 {
                return Err(EvalError::UnsupportedCall);
            }
            let Value::Number(r) = args[0] else {
                return Err(EvalError::UnsupportedCall);
            };
            ("round", vec![("base", base), ("r", Value::Number(r))])
        }
        "union_round" | "union_chamfer" | "union_soft" => {
            if args.len() != 2 {
                return Err(EvalError::UnsupportedCall);
            }
            let Value::Number(r) = args[1] else {
                return Err(EvalError::UnsupportedCall);
            };
            (
                field,
                vec![
                    ("lhs", base),
                    ("rhs", args[0].clone()),
                    ("r", Value::Number(r)),
                ],
            )
        }
        "union_columns" | "union_stairs" => {
            if args.len() != 3 {
                return Err(EvalError::UnsupportedCall);
            }
            let Value::Number(r) = args[1] else {
                return Err(EvalError::UnsupportedCall);
            };
            let Value::Number(n) = args[2] else {
                return Err(EvalError::UnsupportedCall);
            };
            (
                field,
                vec![
                    ("lhs", base),
                    ("rhs", args[0].clone()),
                    ("r", Value::Number(r)),
                    ("n", Value::Number(n)),
                ],
            )
        }
        "intersect_round" | "intersect_chamfer" => {
            if args.len() != 2 {
                return Err(EvalError::UnsupportedCall);
            }
            let Value::Number(r) = args[1] else {
                return Err(EvalError::UnsupportedCall);
            };
            (
                field,
                vec![
                    ("lhs", base),
                    ("rhs", args[0].clone()),
                    ("r", Value::Number(r)),
                ],
            )
        }
        "intersect_columns" | "intersect_stairs" => {
            if args.len() != 3 {
                return Err(EvalError::UnsupportedCall);
            }
            let Value::Number(r) = args[1] else {
                return Err(EvalError::UnsupportedCall);
            };
            let Value::Number(n) = args[2] else {
                return Err(EvalError::UnsupportedCall);
            };
            (
                field,
                vec![
                    ("lhs", base),
                    ("rhs", args[0].clone()),
                    ("r", Value::Number(r)),
                    ("n", Value::Number(n)),
                ],
            )
        }
        "diff_round" | "diff_chamfer" => {
            if args.len() != 2 {
                return Err(EvalError::UnsupportedCall);
            }
            let Value::Number(r) = args[1] else {
                return Err(EvalError::UnsupportedCall);
            };
            (
                field,
                vec![
                    ("lhs", base),
                    ("rhs", args[0].clone()),
                    ("r", Value::Number(r)),
                ],
            )
        }
        "diff_columns" | "diff_stairs" => {
            if args.len() != 3 {
                return Err(EvalError::UnsupportedCall);
            }
            let Value::Number(r) = args[1] else {
                return Err(EvalError::UnsupportedCall);
            };
            let Value::Number(n) = args[2] else {
                return Err(EvalError::UnsupportedCall);
            };
            (
                field,
                vec![
                    ("lhs", base),
                    ("rhs", args[0].clone()),
                    ("r", Value::Number(r)),
                    ("n", Value::Number(n)),
                ],
            )
        }
        "pipe" | "engrave" => {
            if args.len() != 2 {
                return Err(EvalError::UnsupportedCall);
            }
            let Value::Number(r) = args[1] else {
                return Err(EvalError::UnsupportedCall);
            };
            (
                field,
                vec![
                    ("lhs", base),
                    ("rhs", args[0].clone()),
                    ("r", Value::Number(r)),
                ],
            )
        }
        "groove" | "tongue" => {
            if args.len() != 3 {
                return Err(EvalError::UnsupportedCall);
            }
            let Value::Number(ra) = args[1] else {
                return Err(EvalError::UnsupportedCall);
            };
            let Value::Number(rb) = args[2] else {
                return Err(EvalError::UnsupportedCall);
            };
            (
                field,
                vec![
                    ("lhs", base),
                    ("rhs", args[0].clone()),
                    ("ra", Value::Number(ra)),
                    ("rb", Value::Number(rb)),
                ],
            )
        }
        _ => return Ok(None),
    };

    let mut object_fields = HashMap::new();
    for (name, value) in fields {
        object_fields.insert(name.to_string(), value);
    }
    Ok(Some(Value::Object(ObjectValue {
        type_name: Some(type_name.to_string()),
        fields: object_fields,
    })))
}

fn is_sdf_member_operator(field: &str) -> bool {
    matches!(
        field,
        "smooth"
            | "round"
            | "bevel"
            | "chamfer"
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
    )
}

fn eval_unary(op: UnaryOp, value: Value) -> Result<Value, EvalError> {
    match op {
        UnaryOp::Neg => match value {
            Value::Number(v) => Ok(Value::Number(-v)),
            _ => {
                let v = as_vec3(&value).ok_or(EvalError::UnaryTypeMismatch)?;
                Ok(vec3_value([-v[0], -v[1], -v[2]]))
            }
        },
    }
}

fn eval_call(
    callee: &Expr,
    args: &[Expr],
    state: &EvalState,
    locals: &HashMap<String, Value>,
    material_runtime: Option<MaterialRuntime<'_>>,
    top_level_depth: usize,
) -> Result<Value, EvalError> {
    match callee {
        Expr::Ident(name) => {
            let arg_values =
                eval_arg_values(args, state, locals, material_runtime, top_level_depth)?;
            if let Some(value) = eval_ident_call(name, &arg_values)? {
                return Ok(value);
            }
            if let Some(runtime) = material_runtime
                && let Some((params, body)) =
                    runtime.def.statements.iter().find_map(|stmt| match stmt {
                        MaterialStatement::Function {
                            name: fn_name,
                            params,
                            body,
                        } if fn_name == name => Some((params.clone(), body.clone())),
                        _ => None,
                    })
            {
                if runtime.depth >= 32 {
                    return Err(EvalError::MaterialCallDepthExceeded);
                }
                if arg_values.len() != params.len() {
                    return Err(EvalError::UnsupportedCall);
                }
                return eval_material_function_body(
                    state,
                    runtime.def,
                    &params,
                    &body,
                    &arg_values,
                    runtime.depth + 1,
                );
            }
            if let Some(def) = state.function_defs.get(name)
                && let Some(value) =
                    eval_top_level_function_call(state, def, &arg_values, top_level_depth + 1)?
            {
                return Ok(value);
            }
            Err(EvalError::UnsupportedCall)
        }
        Expr::Member { target, field } => {
            if is_sdf_member_operator(field) {
                let base = eval_expr_in_material_scope(
                    target,
                    state,
                    locals,
                    material_runtime,
                    top_level_depth,
                )?;
                let arg_values =
                    eval_arg_values(args, state, locals, material_runtime, top_level_depth)?;
                if let Some(value) = build_sdf_member_value(field, base, arg_values)? {
                    return Ok(value);
                }
            }
            if let Some(name) = flatten_member_expr(callee) {
                let arg_values =
                    eval_arg_values(args, state, locals, material_runtime, top_level_depth)?;
                if let Some(def) = state.function_defs.get(&name)
                    && let Some(value) =
                        eval_top_level_function_call(state, def, &arg_values, top_level_depth + 1)?
                {
                    return Ok(value);
                }
            }
            Err(EvalError::UnsupportedCall)
        }
        _ => Err(EvalError::UnsupportedCall),
    }
}

fn eval_arg_values(
    args: &[Expr],
    state: &EvalState,
    locals: &HashMap<String, Value>,
    material_runtime: Option<MaterialRuntime<'_>>,
    top_level_depth: usize,
) -> Result<Vec<Value>, EvalError> {
    let mut values = Vec::with_capacity(args.len());
    for arg in args {
        values.push(eval_expr_in_material_scope(
            arg,
            state,
            locals,
            material_runtime,
            top_level_depth,
        )?);
    }
    Ok(values)
}

fn eval_ident_call(name: &str, args: &[Value]) -> Result<Option<Value>, EvalError> {
    let value = match name {
        "vec3" => {
            if args.len() != 1 && args.len() != 3 {
                return Err(EvalError::InvalidVec3Call);
            }
            let mut numbers = Vec::with_capacity(args.len());
            for arg in args {
                let Value::Number(value) = arg else {
                    return Err(EvalError::InvalidVec3Call);
                };
                numbers.push(*value);
            }
            let values = if numbers.len() == 1 {
                [numbers[0], numbers[0], numbers[0]]
            } else {
                [numbers[0], numbers[1], numbers[2]]
            };
            vec3_value(values)
        }
        "clamp" => {
            if args.len() != 3 {
                return Err(EvalError::InvalidBuiltinArity {
                    name: "clamp",
                    expected: 3,
                    got: args.len(),
                });
            }
            map_value3("clamp", &args[0], &args[1], &args[2], |x, a, b| {
                let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
                x.clamp(lo, hi)
            })?
        }
        "mix" => {
            if args.len() != 3 {
                return Err(EvalError::InvalidBuiltinArity {
                    name: "mix",
                    expected: 3,
                    got: args.len(),
                });
            }
            map_value3("mix", &args[0], &args[1], &args[2], |x, y, a| {
                x * (1.0 - a) + y * a
            })?
        }
        "step" => {
            if args.len() != 2 {
                return Err(EvalError::InvalidBuiltinArity {
                    name: "step",
                    expected: 2,
                    got: args.len(),
                });
            }
            let Value::Number(edge) = args[0] else {
                return Err(EvalError::BuiltinNumericArgs("step"));
            };
            let Value::Number(x) = args[1] else {
                return Err(EvalError::BuiltinNumericArgs("step"));
            };
            Value::Number(if x < edge { 0.0 } else { 1.0 })
        }
        "smoothstep" => {
            if args.len() != 3 {
                return Err(EvalError::InvalidBuiltinArity {
                    name: "smoothstep",
                    expected: 3,
                    got: args.len(),
                });
            }
            let (Value::Number(edge0), Value::Number(edge1), Value::Number(x)) =
                (&args[0], &args[1], &args[2])
            else {
                return Err(EvalError::BuiltinNumericArgs("smoothstep"));
            };
            let span = edge1 - edge0;
            let t = if span.abs() < f64::EPSILON {
                if x < edge0 { 0.0 } else { 1.0 }
            } else {
                ((x - edge0) / span).clamp(0.0, 1.0)
            };
            Value::Number(t * t * (3.0 - 2.0 * t))
        }
        "saturate" => {
            if args.len() != 1 {
                return Err(EvalError::InvalidBuiltinArity {
                    name: "saturate",
                    expected: 1,
                    got: args.len(),
                });
            }
            map_value1("saturate", &args[0], |x| x.clamp(0.0, 1.0))?
        }
        "abs" => {
            if args.len() != 1 {
                return Err(EvalError::InvalidBuiltinArity {
                    name: "abs",
                    expected: 1,
                    got: args.len(),
                });
            }
            map_value1("abs", &args[0], f64::abs)?
        }
        "floor" => {
            if args.len() != 1 {
                return Err(EvalError::InvalidBuiltinArity {
                    name: "floor",
                    expected: 1,
                    got: args.len(),
                });
            }
            map_value1("floor", &args[0], f64::floor)?
        }
        "ceil" => {
            if args.len() != 1 {
                return Err(EvalError::InvalidBuiltinArity {
                    name: "ceil",
                    expected: 1,
                    got: args.len(),
                });
            }
            map_value1("ceil", &args[0], f64::ceil)?
        }
        "fract" => {
            if args.len() != 1 {
                return Err(EvalError::InvalidBuiltinArity {
                    name: "fract",
                    expected: 1,
                    got: args.len(),
                });
            }
            map_value1("fract", &args[0], |x| x - x.floor())?
        }
        "sqrt" => {
            if args.len() != 1 {
                return Err(EvalError::InvalidBuiltinArity {
                    name: "sqrt",
                    expected: 1,
                    got: args.len(),
                });
            }
            map_value1("sqrt", &args[0], f64::sqrt)?
        }
        "sin" => {
            if args.len() != 1 {
                return Err(EvalError::InvalidBuiltinArity {
                    name: "sin",
                    expected: 1,
                    got: args.len(),
                });
            }
            map_value1("sin", &args[0], f64::sin)?
        }
        "cos" => {
            if args.len() != 1 {
                return Err(EvalError::InvalidBuiltinArity {
                    name: "cos",
                    expected: 1,
                    got: args.len(),
                });
            }
            map_value1("cos", &args[0], f64::cos)?
        }
        "min" => {
            if args.len() != 2 {
                return Err(EvalError::InvalidBuiltinArity {
                    name: "min",
                    expected: 2,
                    got: args.len(),
                });
            }
            map_value2("min", &args[0], &args[1], f64::min)?
        }
        "max" => {
            if args.len() != 2 {
                return Err(EvalError::InvalidBuiltinArity {
                    name: "max",
                    expected: 2,
                    got: args.len(),
                });
            }
            map_value2("max", &args[0], &args[1], f64::max)?
        }
        "pow" => {
            if args.len() != 2 {
                return Err(EvalError::InvalidBuiltinArity {
                    name: "pow",
                    expected: 2,
                    got: args.len(),
                });
            }
            map_value2("pow", &args[0], &args[1], f64::powf)?
        }
        "dot" => {
            if args.len() != 2 {
                return Err(EvalError::InvalidBuiltinArity {
                    name: "dot",
                    expected: 2,
                    got: args.len(),
                });
            }
            let a = as_vec3(&args[0]).ok_or(EvalError::BuiltinVec3Args("dot"))?;
            let b = as_vec3(&args[1]).ok_or(EvalError::BuiltinVec3Args("dot"))?;
            Value::Number(a[0] * b[0] + a[1] * b[1] + a[2] * b[2])
        }
        "length" => {
            if args.len() != 1 {
                return Err(EvalError::InvalidBuiltinArity {
                    name: "length",
                    expected: 1,
                    got: args.len(),
                });
            }
            let v = as_vec3(&args[0]).ok_or(EvalError::BuiltinVec3Args("length"))?;
            Value::Number((v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt())
        }
        "normalize" => {
            if args.len() != 1 {
                return Err(EvalError::InvalidBuiltinArity {
                    name: "normalize",
                    expected: 1,
                    got: args.len(),
                });
            }
            let v = as_vec3(&args[0]).ok_or(EvalError::BuiltinVec3Args("normalize"))?;
            let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            if len <= f64::EPSILON {
                vec3_value([0.0, 0.0, 0.0])
            } else {
                vec3_value([v[0] / len, v[1] / len, v[2] / len])
            }
        }
        _ => return Ok(None),
    };
    Ok(Some(value))
}

pub fn eval_material_function(
    state: &EvalState,
    material_name: &str,
    function_name: &str,
    ctx_value: Value,
) -> Result<Value, EvalError> {
    let def = state
        .material_defs
        .get(material_name)
        .ok_or_else(|| EvalError::UndefinedIdentifier(material_name.to_string()))?;
    let (params, body) = def
        .statements
        .iter()
        .find_map(|stmt| match stmt {
            MaterialStatement::Function { name, params, body } if name == function_name => {
                Some((params.clone(), body.clone()))
            }
            _ => None,
        })
        .ok_or_else(|| EvalError::UndefinedIdentifier(function_name.to_string()))?;
    eval_material_function_body(state, def, &params, &body, &[ctx_value], 0)
}

pub fn eval_top_level_function(
    state: &EvalState,
    function_name: &str,
    arg_values: &[Value],
) -> Result<Value, EvalError> {
    let def = state
        .function_defs
        .get(function_name)
        .ok_or_else(|| EvalError::UndefinedIdentifier(function_name.to_string()))?;
    eval_top_level_function_call(state, def, arg_values, 0)?
        .ok_or_else(|| EvalError::UndefinedIdentifier(function_name.to_string()))
}

pub fn eval_material_properties(
    state: &EvalState,
    material_name: &str,
) -> Result<HashMap<String, Value>, EvalError> {
    let def = state
        .material_defs
        .get(material_name)
        .ok_or_else(|| EvalError::UndefinedIdentifier(material_name.to_string()))?;
    let mut locals = HashMap::new();
    let mut properties = HashMap::new();

    for stmt in &def.statements {
        match stmt {
            MaterialStatement::Binding { name, expr } => {
                let value = eval_expr_in_material_scope(
                    expr,
                    state,
                    &locals,
                    Some(MaterialRuntime { def, depth: 0 }),
                    0,
                )?;
                locals.insert(name.clone(), value);
            }
            MaterialStatement::Property { name, expr } => {
                let value = eval_expr_in_material_scope(
                    expr,
                    state,
                    &locals,
                    Some(MaterialRuntime { def, depth: 0 }),
                    0,
                )?;
                properties.insert(name.clone(), value);
            }
            MaterialStatement::Function { .. } => {}
        }
    }

    Ok(properties)
}

pub fn eval_environment_function(
    state: &EvalState,
    environment_name: &str,
    function_name: &str,
    arg_values: &[Value],
) -> Result<Value, EvalError> {
    let def = state
        .environment_defs
        .get(environment_name)
        .ok_or_else(|| EvalError::UndefinedIdentifier(environment_name.to_string()))?;
    let (params, body) = def
        .statements
        .iter()
        .find_map(|stmt| match stmt {
            MaterialStatement::Function { name, params, body } if name == function_name => {
                Some((params.clone(), body.clone()))
            }
            _ => None,
        })
        .ok_or_else(|| EvalError::UndefinedIdentifier(function_name.to_string()))?;
    eval_environment_function_body(state, def, &params, &body, arg_values, 0)
}

pub fn eval_sdf_function(
    state: &EvalState,
    sdf_name: &str,
    function_name: &str,
    arg_value: Value,
) -> Result<Value, EvalError> {
    let def = state
        .sdf_defs
        .get(sdf_name)
        .ok_or_else(|| EvalError::UndefinedIdentifier(sdf_name.to_string()))?;
    let (params, body) = def
        .statements
        .iter()
        .find_map(|stmt| match stmt {
            SdfStatement::Function { name, params, body } if name == function_name => {
                Some((params.clone(), body.clone()))
            }
            _ => None,
        })
        .ok_or_else(|| EvalError::UndefinedIdentifier(function_name.to_string()))?;
    eval_sdf_function_body(state, def, &params, &body, &[arg_value], 0)
}

pub fn eval_sdf_zero_arg_function(
    state: &EvalState,
    sdf_name: &str,
    function_name: &str,
) -> Result<Value, EvalError> {
    let def = state
        .sdf_defs
        .get(sdf_name)
        .ok_or_else(|| EvalError::UndefinedIdentifier(sdf_name.to_string()))?;
    let (params, body) = def
        .statements
        .iter()
        .find_map(|stmt| match stmt {
            SdfStatement::Function { name, params, body } if name == function_name => {
                Some((params.clone(), body.clone()))
            }
            _ => None,
        })
        .ok_or_else(|| EvalError::UndefinedIdentifier(function_name.to_string()))?;
    if !params.is_empty() {
        return Err(EvalError::UnsupportedCall);
    }
    eval_sdf_function_body(state, def, &params, &body, &[], 0)
}

fn eval_sdf_function_body(
    state: &EvalState,
    def: &SdfDef,
    params: &[String],
    body: &[SdfFunctionStatement],
    arg_values: &[Value],
    depth: usize,
) -> Result<Value, EvalError> {
    let mut locals = HashMap::new();
    if arg_values.len() != params.len() {
        return Err(EvalError::UnsupportedCall);
    }
    for (param, value) in params.iter().zip(arg_values.iter()) {
        locals.insert(param.clone(), value.clone());
    }

    for stmt in &def.statements {
        match stmt {
            SdfStatement::Binding { name, expr } => {
                let value = eval_sdf_expr(expr, state, &locals, Some(SdfRuntime { def, depth }))?;
                locals.insert(name.clone(), value);
            }
            SdfStatement::Function { .. } => {}
        }
    }

    for stmt in body {
        match stmt {
            MaterialFunctionStatement::Binding { name, expr } => {
                let value = eval_sdf_expr(expr, state, &locals, Some(SdfRuntime { def, depth }))?;
                locals.insert(name.clone(), value);
            }
            MaterialFunctionStatement::Return { expr } => {
                return eval_sdf_expr(expr, state, &locals, Some(SdfRuntime { def, depth }));
            }
        }
    }

    Err(EvalError::UndefinedIdentifier(
        "sdf function missing return".to_string(),
    ))
}

fn eval_sdf_expr(
    expr: &Expr,
    state: &EvalState,
    locals: &HashMap<String, Value>,
    sdf_runtime: Option<SdfRuntime<'_>>,
) -> Result<Value, EvalError> {
    match expr {
        Expr::Number(n) => Ok(Value::Number(*n)),
        Expr::Ident(name) => {
            if let Some(value) = locals.get(name) {
                return Ok(value.clone());
            }
            let binding = state
                .bindings
                .get(name)
                .ok_or_else(|| EvalError::UndefinedIdentifier(name.clone()))?;
            Ok(binding.value.clone())
        }
        Expr::ObjectLiteral { type_name, fields } => {
            let mut resolved_fields = HashMap::new();
            for (name, field_expr) in fields {
                resolved_fields.insert(
                    name.clone(),
                    eval_sdf_expr(field_expr, state, locals, sdf_runtime)?,
                );
            }
            Ok(Value::Object(ObjectValue {
                type_name: Some(type_name.clone()),
                fields: resolved_fields,
            }))
        }
        Expr::Binary { lhs, op, rhs } => {
            let left = eval_sdf_expr(lhs, state, locals, sdf_runtime)?;
            let right = eval_sdf_expr(rhs, state, locals, sdf_runtime)?;
            eval_binary(left, *op, right)
        }
        Expr::Member { target, field } => {
            let base = eval_sdf_expr(target, state, locals, sdf_runtime)?;
            let obj = as_object(&base)?;
            obj.fields
                .get(field)
                .cloned()
                .ok_or(EvalError::UndefinedIdentifier(field.clone()))
        }
        Expr::Call { callee, args } => eval_sdf_call(callee, args, state, locals, sdf_runtime),
        Expr::Unary { op, expr } => {
            let value = eval_sdf_expr(expr, state, locals, sdf_runtime)?;
            eval_unary(*op, value)
        }
    }
}

fn eval_sdf_call(
    callee: &Expr,
    args: &[Expr],
    state: &EvalState,
    locals: &HashMap<String, Value>,
    sdf_runtime: Option<SdfRuntime<'_>>,
) -> Result<Value, EvalError> {
    match callee {
        Expr::Ident(name) => {
            let arg_values = eval_sdf_arg_values(args, state, locals, sdf_runtime)?;
            if let Some(value) = eval_ident_call(name, &arg_values)? {
                return Ok(value);
            }
            if let Some(runtime) = sdf_runtime
                && let Some((params, body)) =
                    runtime.def.statements.iter().find_map(|stmt| match stmt {
                        SdfStatement::Function {
                            name: fn_name,
                            params,
                            body,
                        } if fn_name == name => Some((params.clone(), body.clone())),
                        _ => None,
                    })
            {
                if runtime.depth >= 32 {
                    return Err(EvalError::MaterialCallDepthExceeded);
                }
                if arg_values.len() != params.len() {
                    return Err(EvalError::UnsupportedCall);
                }
                return eval_sdf_function_body(
                    state,
                    runtime.def,
                    &params,
                    &body,
                    &arg_values,
                    runtime.depth + 1,
                );
            }
            if let Some(def) = state.function_defs.get(name)
                && let Some(value) = eval_top_level_function_call(
                    state,
                    def,
                    &arg_values,
                    sdf_runtime.map_or(0, |runtime| runtime.depth + 1),
                )?
            {
                return Ok(value);
            }
            Err(EvalError::UnsupportedCall)
        }
        Expr::Member { target, field } => {
            if is_sdf_member_operator(field) {
                let base = eval_sdf_expr(target, state, locals, sdf_runtime)?;
                let arg_values = eval_sdf_arg_values(args, state, locals, sdf_runtime)?;
                if let Some(value) = build_sdf_member_value(field, base, arg_values)? {
                    return Ok(value);
                }
            }
            if let Some(name) = flatten_member_expr(callee) {
                let arg_values = eval_sdf_arg_values(args, state, locals, sdf_runtime)?;
                if let Some(def) = state.function_defs.get(&name)
                    && let Some(value) = eval_top_level_function_call(
                        state,
                        def,
                        &arg_values,
                        sdf_runtime.map_or(0, |runtime| runtime.depth + 1),
                    )?
                {
                    return Ok(value);
                }
            }
            Err(EvalError::UnsupportedCall)
        }
        _ => Err(EvalError::UnsupportedCall),
    }
}

fn eval_sdf_arg_values(
    args: &[Expr],
    state: &EvalState,
    locals: &HashMap<String, Value>,
    sdf_runtime: Option<SdfRuntime<'_>>,
) -> Result<Vec<Value>, EvalError> {
    let mut values = Vec::with_capacity(args.len());
    for arg in args {
        values.push(eval_sdf_expr(arg, state, locals, sdf_runtime)?);
    }
    Ok(values)
}

fn eval_material_function_body(
    state: &EvalState,
    def: &MaterialDef,
    params: &[String],
    body: &[MaterialFunctionStatement],
    arg_values: &[Value],
    depth: usize,
) -> Result<Value, EvalError> {
    let mut locals = HashMap::new();
    if arg_values.len() != params.len() {
        return Err(EvalError::UnsupportedCall);
    }
    for (param, value) in params.iter().zip(arg_values.iter()) {
        locals.insert(param.clone(), value.clone());
    }

    for stmt in &def.statements {
        match stmt {
            MaterialStatement::Binding { name, expr } => {
                let value = eval_expr_in_material_scope(
                    expr,
                    state,
                    &locals,
                    Some(MaterialRuntime { def, depth }),
                    depth,
                )?;
                locals.insert(name.clone(), value);
            }
            MaterialStatement::Property { .. } | MaterialStatement::Function { .. } => {}
        }
    }

    for stmt in body {
        match stmt {
            MaterialFunctionStatement::Binding { name, expr } => {
                let value = eval_expr_in_material_scope(
                    expr,
                    state,
                    &locals,
                    Some(MaterialRuntime { def, depth }),
                    depth,
                )?;
                locals.insert(name.clone(), value);
            }
            MaterialFunctionStatement::Return { expr } => {
                return eval_expr_in_material_scope(
                    expr,
                    state,
                    &locals,
                    Some(MaterialRuntime { def, depth }),
                    depth,
                );
            }
        }
    }

    Err(EvalError::UndefinedIdentifier(
        "material function missing return".to_string(),
    ))
}

fn eval_environment_function_body(
    state: &EvalState,
    def: &EnvironmentDef,
    params: &[String],
    body: &[MaterialFunctionStatement],
    arg_values: &[Value],
    depth: usize,
) -> Result<Value, EvalError> {
    let mut locals = HashMap::new();
    if arg_values.len() != params.len() {
        return Err(EvalError::UnsupportedCall);
    }
    for (param, value) in params.iter().zip(arg_values.iter()) {
        locals.insert(param.clone(), value.clone());
    }

    for stmt in &def.statements {
        match stmt {
            MaterialStatement::Binding { name, expr } => {
                let value = eval_expr_in_environment_scope(expr, state, &locals, def, depth)?;
                locals.insert(name.clone(), value);
            }
            MaterialStatement::Property { .. } | MaterialStatement::Function { .. } => {}
        }
    }

    for stmt in body {
        match stmt {
            MaterialFunctionStatement::Binding { name, expr } => {
                let value = eval_expr_in_environment_scope(expr, state, &locals, def, depth)?;
                locals.insert(name.clone(), value);
            }
            MaterialFunctionStatement::Return { expr } => {
                return eval_expr_in_environment_scope(expr, state, &locals, def, depth);
            }
        }
    }

    Err(EvalError::UndefinedIdentifier(
        "environment function missing return".to_string(),
    ))
}

fn eval_expr_in_environment_scope(
    expr: &Expr,
    state: &EvalState,
    locals: &HashMap<String, Value>,
    def: &EnvironmentDef,
    depth: usize,
) -> Result<Value, EvalError> {
    match expr {
        Expr::Call { callee, args } => match callee.as_ref() {
            Expr::Ident(name) => {
                let arg_values = args
                    .iter()
                    .map(|arg| eval_expr_in_environment_scope(arg, state, locals, def, depth))
                    .collect::<Result<Vec<_>, _>>()?;
                if let Some(value) = eval_ident_call(name, &arg_values)? {
                    return Ok(value);
                }
                if let Some((params, body)) = def.statements.iter().find_map(|stmt| match stmt {
                    MaterialStatement::Function {
                        name: fn_name,
                        params,
                        body,
                    } if fn_name == name => Some((params.clone(), body.clone())),
                    _ => None,
                }) {
                    if depth >= 32 {
                        return Err(EvalError::MaterialCallDepthExceeded);
                    }
                    return eval_environment_function_body(
                        state,
                        def,
                        &params,
                        &body,
                        &arg_values,
                        depth + 1,
                    );
                }
                if let Some(top) = state.function_defs.get(name)
                    && let Some(value) =
                        eval_top_level_function_call(state, top, &arg_values, depth + 1)?
                {
                    return Ok(value);
                }
                Err(EvalError::UnsupportedCall)
            }
            Expr::Member { .. } => {
                let flattened = flatten_member_expr(callee).ok_or(EvalError::UnsupportedCall)?;
                let arg_values = args
                    .iter()
                    .map(|arg| eval_expr_in_environment_scope(arg, state, locals, def, depth))
                    .collect::<Result<Vec<_>, _>>()?;
                if let Some(top) = state.function_defs.get(&flattened)
                    && let Some(value) =
                        eval_top_level_function_call(state, top, &arg_values, depth + 1)?
                {
                    return Ok(value);
                }
                Err(EvalError::UnsupportedCall)
            }
            _ => Err(EvalError::UnsupportedCall),
        },
        _ => eval_expr_in_material_scope(expr, state, locals, None, depth),
    }
}

fn eval_top_level_function_call(
    state: &EvalState,
    def: &FunctionDef,
    arg_values: &[Value],
    depth: usize,
) -> Result<Option<Value>, EvalError> {
    if arg_values.len() != def.params.len() {
        return Err(EvalError::UnsupportedCall);
    }
    if depth >= 32 {
        return Err(EvalError::MaterialCallDepthExceeded);
    }

    let mut locals = HashMap::new();
    for (param, value) in def.params.iter().zip(arg_values.iter()) {
        locals.insert(param.clone(), value.clone());
    }

    for stmt in &def.body {
        match stmt {
            MaterialFunctionStatement::Binding { name, expr } => {
                let value = eval_expr_in_material_scope(expr, state, &locals, None, depth)?;
                locals.insert(name.clone(), value);
            }
            MaterialFunctionStatement::Return { expr } => {
                return Ok(Some(eval_expr_in_material_scope(
                    expr, state, &locals, None, depth,
                )?));
            }
        }
    }

    Err(EvalError::UndefinedIdentifier(
        "function missing return".to_string(),
    ))
}

fn map_value1(
    name: &'static str,
    value: &Value,
    f: impl Fn(f64) -> f64,
) -> Result<Value, EvalError> {
    match value {
        Value::Number(x) => Ok(Value::Number(f(*x))),
        _ => {
            let v = as_vec3(value).ok_or(EvalError::BuiltinNumericOrVec3Args(name))?;
            Ok(vec3_value([f(v[0]), f(v[1]), f(v[2])]))
        }
    }
}

fn map_value2(
    name: &'static str,
    a: &Value,
    b: &Value,
    f: impl Fn(f64, f64) -> f64,
) -> Result<Value, EvalError> {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => Ok(Value::Number(f(*x, *y))),
        _ => {
            let va = as_broadcastable_vec3(a).ok_or(EvalError::BuiltinNumericOrVec3Args(name))?;
            let vb = as_broadcastable_vec3(b).ok_or(EvalError::BuiltinNumericOrVec3Args(name))?;
            Ok(vec3_value([
                f(va[0], vb[0]),
                f(va[1], vb[1]),
                f(va[2], vb[2]),
            ]))
        }
    }
}

fn map_value3(
    name: &'static str,
    a: &Value,
    b: &Value,
    c: &Value,
    f: impl Fn(f64, f64, f64) -> f64,
) -> Result<Value, EvalError> {
    match (a, b, c) {
        (Value::Number(x), Value::Number(y), Value::Number(z)) => Ok(Value::Number(f(*x, *y, *z))),
        _ => {
            let va = as_broadcastable_vec3(a).ok_or(EvalError::BuiltinNumericOrVec3Args(name))?;
            let vb = as_broadcastable_vec3(b).ok_or(EvalError::BuiltinNumericOrVec3Args(name))?;
            let vc = as_broadcastable_vec3(c).ok_or(EvalError::BuiltinNumericOrVec3Args(name))?;
            Ok(vec3_value([
                f(va[0], vb[0], vc[0]),
                f(va[1], vb[1], vc[1]),
                f(va[2], vb[2], vc[2]),
            ]))
        }
    }
}

fn as_broadcastable_vec3(value: &Value) -> Option<[f64; 3]> {
    match value {
        Value::Number(x) => Some([*x, *x, *x]),
        _ => as_vec3(value),
    }
}

fn as_vec3(value: &Value) -> Option<[f64; 3]> {
    let Value::Object(obj) = value else {
        return None;
    };
    let x = match obj.fields.get("x")? {
        Value::Number(v) => *v,
        _ => return None,
    };
    let y = match obj.fields.get("y")? {
        Value::Number(v) => *v,
        _ => return None,
    };
    let z = match obj.fields.get("z")? {
        Value::Number(v) => *v,
        _ => return None,
    };
    Some([x, y, z])
}

fn vec3_value(v: [f64; 3]) -> Value {
    let mut fields = HashMap::new();
    fields.insert("x".to_string(), Value::Number(v[0]));
    fields.insert("y".to_string(), Value::Number(v[1]));
    fields.insert("z".to_string(), Value::Number(v[2]));
    Value::Object(ObjectValue {
        type_name: Some("vec3".to_string()),
        fields,
    })
}

fn as_object(value: &Value) -> Result<&ObjectValue, EvalError> {
    let Value::Object(obj) = value else {
        return Err(EvalError::MemberAccessOnNonObject);
    };
    Ok(obj)
}

fn as_object_mut(value: &mut Value) -> Result<&mut ObjectValue, EvalError> {
    let Value::Object(obj) = value else {
        return Err(EvalError::MemberAccessOnNonObject);
    };
    Ok(obj)
}
