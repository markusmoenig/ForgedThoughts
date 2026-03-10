mod ast;
mod eval;
mod lexer;
mod materials;
mod parser;
mod render_api;
mod renderer;

use std::{
    env, fs,
    path::{Path, PathBuf},
};

pub use ast::{BinaryOp, Expr, Program, Statement, UnaryOp};
pub use eval::{
    Binding, EvalError, EvalState, ObjectValue, Value, eval_material_function,
    eval_material_properties, eval_program,
};
pub use materials::{
    BsdfSample as MaterialBsdfSample, ColorPattern, DielectricMaterial, LambertMaterial, Material,
    MaterialBsdf, MaterialParams, MediumParams, MetalMaterial, SampleInput as MaterialSampleInput,
    SubsurfaceParams,
};
pub use parser::{ParseError, parse_program};
pub use render_api::{
    Bsdf, Camera, CameraKind, EnvLight, Integrator, Light, LightSample, MaterialKind,
    MaterialModel, OpenPbrMaterial, PinholeCamera, PointLight, PreviewIntegrator, Ray, Spectrum,
    SurfaceHit, Vec3,
};
pub use renderer::{
    AccelMode, PathtraceProgress, PathtraceSettings, RayDebugAov, RayProgress, RaySettings,
    RenderError, RenderOptions, SceneRenderSettings, extract_scene_render_settings,
    render_depth_png, render_depth_png_with_accel, render_pathtrace_png_with_accel,
    render_pathtrace_progressive_with_accel, render_ray_png_with_accel,
    render_ray_progressive_with_accel,
};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub default_scene_path: Option<PathBuf>,
}

impl AppConfig {
    #[must_use]
    pub fn from_env() -> Self {
        let default_scene_path = env::var_os("FORGEDTHOUGHTS_SCENE").map(PathBuf::from);
        Self { default_scene_path }
    }
}

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("missing scene input; pass --scene or set FORGEDTHOUGHTS_SCENE")]
    MissingSceneInput,
    #[error("failed to read scene file {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("parse error: {0}")]
    Parse(#[from] ParseError),
    #[error("eval error: {0}")]
    Eval(#[from] EvalError),
    #[error("render error: {0}")]
    Render(#[from] RenderError),
}

pub fn resolve_scene_path(
    cli_scene: Option<PathBuf>,
    cfg: &AppConfig,
) -> Result<PathBuf, CoreError> {
    cli_scene
        .or_else(|| cfg.default_scene_path.clone())
        .ok_or(CoreError::MissingSceneInput)
}

pub fn load_and_eval_scene(scene_path: &Path) -> Result<EvalState, CoreError> {
    let source = fs::read_to_string(scene_path).map_err(|source| CoreError::Io {
        path: scene_path.to_path_buf(),
        source,
    })?;
    let program = parse_program(&source)?;
    let state = eval_program(&program)?;
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::{Value, eval_program, parse_program};

    #[test]
    fn parses_and_evaluates_mvp_language_subset() {
        let source = r#"
            var s = Sphere{};
            s.x = 10;
            s.material.roughness = 0.5;

            let mat = Material {
              metallic: 0.4
            };

            var d = Box {
              size: vec3(1.0, 1.0, 1.0),
              material: mat
            };

            let b = (s-d).smooth(0.2);
        "#;

        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");

        let s = &state
            .bindings
            .get("s")
            .expect("binding s should exist")
            .value;
        let Value::Object(s_obj) = s else {
            panic!("s should be an object");
        };
        assert!(s_obj.fields.contains_key("x"));
        assert!(s_obj.fields.contains_key("material"));

        let d = &state
            .bindings
            .get("d")
            .expect("binding d should exist")
            .value;
        let Value::Object(d_obj) = d else {
            panic!("d should be an object");
        };
        assert!(d_obj.fields.contains_key("size"));
        assert!(d_obj.fields.contains_key("material"));

        assert!(state.bindings.contains_key("b"));
    }

    #[test]
    fn rejects_assignment_to_immutable_binding() {
        let source = r#"
            let a = Sphere{};
            a.x = 2;
        "#;

        let program = parse_program(source).expect("program should parse");
        let result = eval_program(&program);
        assert!(result.is_err());
    }

    #[test]
    fn evaluates_unary_minus_number_expressions() {
        let source = r#"
            let a = -3.5;
            let b = 1 + -2;
        "#;
        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");

        let a = &state.bindings.get("a").expect("a binding").value;
        let b = &state.bindings.get("b").expect("b binding").value;
        assert_eq!(a, &Value::Number(-3.5));
        assert_eq!(b, &Value::Number(-1.0));
    }

    #[test]
    fn supports_negative_vec3_arguments() {
        let source = r#"
            let v = vec3(-3.0, 2.0, -1.0);
        "#;
        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");
        let value = &state.bindings.get("v").expect("v binding").value;
        let Value::Object(v_obj) = value else {
            panic!("v should be an object");
        };
        assert_eq!(v_obj.fields.get("x"), Some(&Value::Number(-3.0)));
        assert_eq!(v_obj.fields.get("z"), Some(&Value::Number(-1.0)));
    }

    #[test]
    fn supports_scalar_vec3_broadcast() {
        let source = r#"
            let v = vec3(0.25);
        "#;
        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");
        let value = &state.bindings.get("v").expect("v binding").value;
        let Value::Object(v_obj) = value else {
            panic!("v should be an object");
        };
        assert_eq!(v_obj.fields.get("x"), Some(&Value::Number(0.25)));
        assert_eq!(v_obj.fields.get("y"), Some(&Value::Number(0.25)));
        assert_eq!(v_obj.fields.get("z"), Some(&Value::Number(0.25)));
    }

    #[test]
    fn supports_round_operator_calls() {
        let source = r#"
            var s = Sphere{};
            let r = s.round(0.1);
        "#;
        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");
        let value = &state.bindings.get("r").expect("r binding").value;
        let Value::Object(obj) = value else {
            panic!("r should be an object");
        };
        assert_eq!(obj.type_name.as_deref(), Some("round"));
        assert!(obj.fields.contains_key("base"));
        assert_eq!(obj.fields.get("r"), Some(&Value::Number(0.1)));
    }

    #[test]
    fn supports_nested_transform_assignments() {
        let source = r#"
            var s = Sphere{};
            s.pos.x = 1.5;
            s.pos.y = -0.25;
            s.rot.z = 30.0;
        "#;
        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");
        let value = &state.bindings.get("s").expect("s binding").value;
        let Value::Object(obj) = value else {
            panic!("s should be an object");
        };
        let Value::Object(pos) = obj.fields.get("pos").expect("pos object") else {
            panic!("pos should be an object");
        };
        let Value::Object(rot) = obj.fields.get("rot").expect("rot object") else {
            panic!("rot should be an object");
        };
        assert_eq!(pos.fields.get("x"), Some(&Value::Number(1.5)));
        assert_eq!(pos.fields.get("y"), Some(&Value::Number(-0.25)));
        assert_eq!(rot.fields.get("z"), Some(&Value::Number(30.0)));
    }
}
