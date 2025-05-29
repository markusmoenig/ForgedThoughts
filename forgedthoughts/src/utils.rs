use crate::F;
use vek::{Vec2, Vec3, Vec4};

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
