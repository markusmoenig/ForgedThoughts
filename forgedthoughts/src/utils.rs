use crate::{NodeTerminal, NodeTerminalRole, F};
use vek::{Vec2, Vec3, Vec4};

use wasmer::Value;

/// Convert an array of wasmer values to a Vec4
pub fn values_to_vec4(values: &[Value]) -> Vec4<F> {
    let arr = values_to_array4(values);
    Vec4::new(arr[0], arr[1], arr[2], arr[3])
}

/// Convert an array of wasmer values into an array of F
pub fn values_to_array4(values: &[Value]) -> [F; 4] {
    #[cfg(feature = "double")]
    {
        [
            values.first().and_then(Value::f64).unwrap_or(0.0) as F,
            values.get(1).and_then(Value::f64).unwrap_or(0.0) as F,
            values.get(2).and_then(Value::f64).unwrap_or(0.0) as F,
            values.get(3).and_then(Value::f64).unwrap_or(0.0) as F,
        ]
    }

    #[cfg(not(feature = "double"))]
    {
        [
            values.first().and_then(Value::f32).unwrap_or(0.0) as F,
            values.get(1).and_then(Value::f32).unwrap_or(0.0) as F,
            values.get(2).and_then(Value::f32).unwrap_or(0.0) as F,
            values.get(3).and_then(Value::f32).unwrap_or(0.0) as F,
        ]
    }
}

pub fn parse_float(s: &str) -> F {
    s.trim().parse::<F>().unwrap_or(0.0)
}

pub fn parse_vec2(s: &str) -> Vec2<F> {
    let content = s
        .trim()
        .trim_start_matches("vec2")
        .trim_start_matches('(')
        .trim_end_matches(')');
    let parts: Vec<&str> = content.split(',').map(|p| p.trim()).collect();
    if parts.len() == 2 {
        Vec2::new(
            parts[0].parse::<F>().unwrap_or(0.0),
            parts[1].parse::<F>().unwrap_or(0.0),
        )
    } else {
        Vec2::zero()
    }
}

pub fn parse_vec3(s: &str) -> Vec3<F> {
    let content = s
        .trim()
        .trim_start_matches("vec3")
        .trim_start_matches('(')
        .trim_end_matches(')');
    let parts: Vec<&str> = content.split(',').map(|p| p.trim()).collect();
    if parts.len() == 3 {
        Vec3::new(
            parts[0].parse::<F>().unwrap_or(0.0),
            parts[1].parse::<F>().unwrap_or(0.0),
            parts[2].parse::<F>().unwrap_or(0.0),
        )
    } else {
        Vec3::zero()
    }
}

pub fn parse_vec4(s: &str) -> Vec4<F> {
    let content = s
        .trim()
        .trim_start_matches("vec4")
        .trim_start_matches('(')
        .trim_end_matches(')');
    let parts: Vec<&str> = content.split(',').map(|p| p.trim()).collect();
    if parts.len() == 4 {
        Vec4::new(
            parts[0].parse::<F>().unwrap_or(0.0),
            parts[1].parse::<F>().unwrap_or(0.0),
            parts[2].parse::<F>().unwrap_or(0.0),
            parts[3].parse::<F>().unwrap_or(0.0),
        )
    } else {
        Vec4::zero()
    }
}

pub fn build_uv_args(pos: Vec2<F>, size: Vec2<F>) -> Vec<Value> {
    #[cfg(feature = "double")]
    return vec![
        Value::F64(pos.x as f64 / size.x as f64),
        Value::F64((size.y as f64 - pos.y as f64) / size.y as f64),
        Value::F64(size.x as f64),
        Value::F64(size.y as f64),
    ];

    #[cfg(not(feature = "double"))]
    return vec![
        Value::F32(pos.x),
        Value::F32(pos.y),
        Value::F32(size.x),
        Value::F32(size.y),
    ];
}

pub fn push_terminal_value(args: &mut Vec<Value>, input: &NodeTerminalRole) {
    match input {
        NodeTerminalRole::Vec1(v) => {
            #[cfg(feature = "double")]
            args.push(Value::F64(*v));
            #[cfg(not(feature = "double"))]
            args.push(Value::F32(*v));
        }
        NodeTerminalRole::Vec2(v) => {
            #[cfg(feature = "double")]
            {
                args.push(Value::F64(v.x));
                args.push(Value::F64(v.y));
            }
            #[cfg(not(feature = "double"))]
            {
                args.push(Value::F32(v.x));
                args.push(Value::F32(v.y));
            }
        }
        NodeTerminalRole::Vec3(v) => {
            #[cfg(feature = "double")]
            {
                args.push(Value::F64(v.x));
                args.push(Value::F64(v.y));
                args.push(Value::F64(v.z));
            }
            #[cfg(not(feature = "double"))]
            {
                args.push(Value::F32(v.x));
                args.push(Value::F32(v.y));
                args.push(Value::F32(v.z));
            }
        }
        NodeTerminalRole::Vec4(v) => {
            #[cfg(feature = "double")]
            {
                args.push(Value::F64(v.x));
                args.push(Value::F64(v.y));
                args.push(Value::F64(v.z));
                args.push(Value::F64(v.w));
            }
            #[cfg(not(feature = "double"))]
            {
                args.push(Value::F32(v.x));
                args.push(Value::F32(v.y));
                args.push(Value::F32(v.z));
                args.push(Value::F32(v.w));
            }
        }
    }
}

pub fn extract_terminal_from_output(
    vec: &Vec4<F>,
    output_name: &str,
    outputs: &Vec<NodeTerminal>,
) -> Result<NodeTerminalRole, String> {
    for (i, terminal) in outputs.iter().enumerate() {
        if terminal.name == output_name {
            return Ok(terminal.role.extract_from_vec4(*vec, &terminal.swizzle));
        }
    }
    Err(format!(
        "Output '{}' not found in terminal list",
        output_name
    ))
}
