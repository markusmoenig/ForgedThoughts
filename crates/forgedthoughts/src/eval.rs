use std::collections::HashMap;

use thiserror::Error;

use crate::ast::{
    BinaryOp, Expr, MaterialDef, MaterialFunctionStatement, MaterialStatement, Program, Statement,
    UnaryOp,
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
    pub material_defs: HashMap<String, MaterialDef>,
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
    #[error("unary operation requires numeric operand")]
    UnaryTypeMismatch,
    #[error("binary operation requires numeric operands")]
    BinaryTypeMismatch,
}

pub fn eval_program(program: &Program) -> Result<EvalState, EvalError> {
    let mut state = EvalState {
        bindings: HashMap::new(),
        material_defs: HashMap::new(),
    };

    for stmt in &program.statements {
        eval_statement(stmt, &mut state)?;
    }

    Ok(state)
}

fn eval_statement(stmt: &Statement, state: &mut EvalState) -> Result<(), EvalError> {
    match stmt {
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
        Statement::MaterialDef(def) => {
            state.material_defs.insert(def.name.clone(), def.clone());
            Ok(())
        }
    }
}

fn assign_path(path: &[String], value: Value, state: &mut EvalState) -> Result<(), EvalError> {
    let Some((root, rest)) = path.split_first() else {
        return Ok(());
    };

    let binding = state
        .bindings
        .get_mut(root)
        .ok_or_else(|| EvalError::UndefinedIdentifier(root.clone()))?;

    if !binding.mutable {
        return Err(EvalError::ImmutableBinding(root.clone()));
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
    eval_expr_with_scope(expr, state, &HashMap::new())
}

fn eval_expr_with_scope(
    expr: &Expr,
    state: &EvalState,
    locals: &HashMap<String, Value>,
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
                    eval_expr_with_scope(field_expr, state, locals)?,
                );
            }
            Ok(Value::Object(ObjectValue {
                type_name: Some(type_name.clone()),
                fields: resolved_fields,
            }))
        }
        Expr::Binary { lhs, op, rhs } => {
            let left = eval_expr_with_scope(lhs, state, locals)?;
            let right = eval_expr_with_scope(rhs, state, locals)?;
            eval_binary(left, *op, right)
        }
        Expr::Member { target, field } => {
            let base = eval_expr_with_scope(target, state, locals)?;
            let obj = as_object(&base)?;
            obj.fields
                .get(field)
                .cloned()
                .ok_or(EvalError::UndefinedIdentifier(field.clone()))
        }
        Expr::Call { callee, args } => eval_call(callee, args, state, locals),
        Expr::Unary { op, expr } => {
            let value = eval_expr_with_scope(expr, state, locals)?;
            eval_unary(*op, value)
        }
    }
}

fn eval_binary(lhs: Value, op: BinaryOp, rhs: Value) -> Result<Value, EvalError> {
    if let (Value::Number(left), Value::Number(right)) = (&lhs, &rhs) {
        let out = match op {
            BinaryOp::Add => left + right,
            BinaryOp::Sub => left - right,
            BinaryOp::Mul => left * right,
            BinaryOp::Div => left / right,
        };
        return Ok(Value::Number(out));
    }

    match op {
        BinaryOp::Add | BinaryOp::Sub => {
            let op_name = match op {
                BinaryOp::Add => "add",
                BinaryOp::Sub => "sub",
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

fn eval_unary(op: UnaryOp, value: Value) -> Result<Value, EvalError> {
    match op {
        UnaryOp::Neg => match value {
            Value::Number(v) => Ok(Value::Number(-v)),
            _ => Err(EvalError::UnaryTypeMismatch),
        },
    }
}

fn eval_call(
    callee: &Expr,
    args: &[Expr],
    state: &EvalState,
    locals: &HashMap<String, Value>,
) -> Result<Value, EvalError> {
    match callee {
        Expr::Ident(name) if name == "vec3" => {
            if args.len() != 1 && args.len() != 3 {
                return Err(EvalError::InvalidVec3Call);
            }

            let mut values = Vec::with_capacity(args.len());
            for arg in args {
                match eval_expr_with_scope(arg, state, locals)? {
                    Value::Number(value) => values.push(value),
                    _ => return Err(EvalError::InvalidVec3Call),
                }
            }

            let values = if values.len() == 1 {
                [values[0], values[0], values[0]]
            } else {
                [values[0], values[1], values[2]]
            };

            let mut fields = HashMap::new();
            fields.insert("x".to_string(), Value::Number(values[0]));
            fields.insert("y".to_string(), Value::Number(values[1]));
            fields.insert("z".to_string(), Value::Number(values[2]));

            Ok(Value::Object(ObjectValue {
                type_name: Some("vec3".to_string()),
                fields,
            }))
        }
        Expr::Ident(name) if name == "clamp" => {
            let values = eval_builtin_values("clamp", args, state, locals, 3)?;
            map_value3("clamp", &values[0], &values[1], &values[2], |x, a, b| {
                let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
                x.clamp(lo, hi)
            })
        }
        Expr::Ident(name) if name == "mix" => {
            let values = eval_builtin_values("mix", args, state, locals, 3)?;
            map_value3("mix", &values[0], &values[1], &values[2], |x, y, a| {
                x * (1.0 - a) + y * a
            })
        }
        Expr::Ident(name) if name == "step" => {
            let values = eval_builtin_numbers("step", args, state, locals, 2)?;
            let (edge, x) = (values[0], values[1]);
            Ok(Value::Number(if x < edge { 0.0 } else { 1.0 }))
        }
        Expr::Ident(name) if name == "smoothstep" => {
            let values = eval_builtin_numbers("smoothstep", args, state, locals, 3)?;
            let (edge0, edge1, x) = (values[0], values[1], values[2]);
            let span = edge1 - edge0;
            let t = if span.abs() < f64::EPSILON {
                if x < edge0 { 0.0 } else { 1.0 }
            } else {
                ((x - edge0) / span).clamp(0.0, 1.0)
            };
            Ok(Value::Number(t * t * (3.0 - 2.0 * t)))
        }
        Expr::Ident(name) if name == "saturate" => {
            let values = eval_builtin_values("saturate", args, state, locals, 1)?;
            map_value1("saturate", &values[0], |x| x.clamp(0.0, 1.0))
        }
        Expr::Ident(name) if name == "abs" => {
            let values = eval_builtin_values("abs", args, state, locals, 1)?;
            map_value1("abs", &values[0], f64::abs)
        }
        Expr::Ident(name) if name == "floor" => {
            let values = eval_builtin_values("floor", args, state, locals, 1)?;
            map_value1("floor", &values[0], f64::floor)
        }
        Expr::Ident(name) if name == "ceil" => {
            let values = eval_builtin_values("ceil", args, state, locals, 1)?;
            map_value1("ceil", &values[0], f64::ceil)
        }
        Expr::Ident(name) if name == "fract" => {
            let values = eval_builtin_values("fract", args, state, locals, 1)?;
            map_value1("fract", &values[0], |x| x - x.floor())
        }
        Expr::Ident(name) if name == "sqrt" => {
            let values = eval_builtin_values("sqrt", args, state, locals, 1)?;
            map_value1("sqrt", &values[0], f64::sqrt)
        }
        Expr::Ident(name) if name == "sin" => {
            let values = eval_builtin_values("sin", args, state, locals, 1)?;
            map_value1("sin", &values[0], f64::sin)
        }
        Expr::Ident(name) if name == "cos" => {
            let values = eval_builtin_values("cos", args, state, locals, 1)?;
            map_value1("cos", &values[0], f64::cos)
        }
        Expr::Ident(name) if name == "min" => {
            let values = eval_builtin_values("min", args, state, locals, 2)?;
            map_value2("min", &values[0], &values[1], f64::min)
        }
        Expr::Ident(name) if name == "max" => {
            let values = eval_builtin_values("max", args, state, locals, 2)?;
            map_value2("max", &values[0], &values[1], f64::max)
        }
        Expr::Ident(name) if name == "pow" => {
            let values = eval_builtin_values("pow", args, state, locals, 2)?;
            map_value2("pow", &values[0], &values[1], f64::powf)
        }
        Expr::Ident(name) if name == "dot" => {
            let values = eval_builtin_values("dot", args, state, locals, 2)?;
            let a = as_vec3(&values[0]).ok_or(EvalError::BuiltinVec3Args("dot"))?;
            let b = as_vec3(&values[1]).ok_or(EvalError::BuiltinVec3Args("dot"))?;
            Ok(Value::Number(a[0] * b[0] + a[1] * b[1] + a[2] * b[2]))
        }
        Expr::Ident(name) if name == "length" => {
            let values = eval_builtin_values("length", args, state, locals, 1)?;
            let v = as_vec3(&values[0]).ok_or(EvalError::BuiltinVec3Args("length"))?;
            Ok(Value::Number(
                (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt(),
            ))
        }
        Expr::Ident(name) if name == "normalize" => {
            let values = eval_builtin_values("normalize", args, state, locals, 1)?;
            let v = as_vec3(&values[0]).ok_or(EvalError::BuiltinVec3Args("normalize"))?;
            let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            if len <= f64::EPSILON {
                Ok(vec3_value([0.0, 0.0, 0.0]))
            } else {
                Ok(vec3_value([v[0] / len, v[1] / len, v[2] / len]))
            }
        }
        Expr::Member { target, field } if field == "smooth" => {
            if args.len() != 1 {
                return Err(EvalError::UnsupportedCall);
            }

            let base = eval_expr_with_scope(target, state, locals)?;
            let radius = eval_expr_with_scope(&args[0], state, locals)?;
            let Value::Number(k) = radius else {
                return Err(EvalError::UnsupportedCall);
            };

            let mut fields = HashMap::new();
            fields.insert("base".to_string(), base);
            fields.insert("k".to_string(), Value::Number(k));
            Ok(Value::Object(ObjectValue {
                type_name: Some("smooth".to_string()),
                fields,
            }))
        }
        Expr::Member { target, field }
            if field == "round" || field == "bevel" || field == "chamfer" =>
        {
            if args.len() != 1 {
                return Err(EvalError::UnsupportedCall);
            }

            let base = eval_expr_with_scope(target, state, locals)?;
            let radius = eval_expr_with_scope(&args[0], state, locals)?;
            let Value::Number(r) = radius else {
                return Err(EvalError::UnsupportedCall);
            };

            let mut fields = HashMap::new();
            fields.insert("base".to_string(), base);
            fields.insert("r".to_string(), Value::Number(r));
            Ok(Value::Object(ObjectValue {
                type_name: Some("round".to_string()),
                fields,
            }))
        }
        _ => Err(EvalError::UnsupportedCall),
    }
}

fn eval_builtin_numbers(
    name: &'static str,
    args: &[Expr],
    state: &EvalState,
    locals: &HashMap<String, Value>,
    expected_arity: usize,
) -> Result<Vec<f64>, EvalError> {
    if args.len() != expected_arity {
        return Err(EvalError::InvalidBuiltinArity {
            name,
            expected: expected_arity,
            got: args.len(),
        });
    }

    let mut values = Vec::with_capacity(expected_arity);
    for arg in args {
        let Value::Number(n) = eval_expr_with_scope(arg, state, locals)? else {
            return Err(EvalError::BuiltinNumericArgs(name));
        };
        values.push(n);
    }
    Ok(values)
}

fn eval_builtin_values(
    name: &'static str,
    args: &[Expr],
    state: &EvalState,
    locals: &HashMap<String, Value>,
    expected_arity: usize,
) -> Result<Vec<Value>, EvalError> {
    if args.len() != expected_arity {
        return Err(EvalError::InvalidBuiltinArity {
            name,
            expected: expected_arity,
            got: args.len(),
        });
    }

    let mut values = Vec::with_capacity(expected_arity);
    for arg in args {
        values.push(eval_expr_with_scope(arg, state, locals)?);
    }
    Ok(values)
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
    let mut locals = HashMap::new();

    let (param_name, body) = def
        .statements
        .iter()
        .find_map(|stmt| match stmt {
            MaterialStatement::Function { name, param, body } if name == function_name => {
                Some((param.clone(), body.clone()))
            }
            _ => None,
        })
        .ok_or_else(|| EvalError::UndefinedIdentifier(function_name.to_string()))?;

    locals.insert(param_name, ctx_value);

    for stmt in &def.statements {
        match stmt {
            MaterialStatement::Binding { name, expr } => {
                let value = eval_expr_with_scope(expr, state, &locals)?;
                locals.insert(name.clone(), value);
            }
            MaterialStatement::Property { .. } | MaterialStatement::Function { .. } => {}
        }
    }

    for stmt in &body {
        match stmt {
            MaterialFunctionStatement::Binding { name, expr } => {
                let value = eval_expr_with_scope(expr, state, &locals)?;
                locals.insert(name.clone(), value);
            }
            MaterialFunctionStatement::Return { expr } => {
                return eval_expr_with_scope(expr, state, &locals);
            }
        }
    }

    Err(EvalError::UndefinedIdentifier(format!(
        "material function '{function_name}' missing return"
    )))
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
                let value = eval_expr_with_scope(expr, state, &locals)?;
                locals.insert(name.clone(), value);
            }
            MaterialStatement::Property { name, expr } => {
                let value = eval_expr_with_scope(expr, state, &locals)?;
                properties.insert(name.clone(), value);
            }
            MaterialStatement::Function { .. } => {}
        }
    }

    Ok(properties)
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
