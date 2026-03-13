mod ast;
mod eval;
mod jit;
mod lexer;
mod materials;
mod parser;
mod render_api;
mod renderer;
mod vm;

use std::{
    collections::{HashMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
};

pub use ast::{BinaryOp, Expr, Program, Statement, UnaryOp};
pub use eval::{
    Binding, EvalError, EvalState, FunctionValue, ObjectValue, Value, eval_environment_function,
    eval_function_value, eval_material_function, eval_material_function_with_overrides,
    eval_material_properties, eval_material_properties_with_overrides, eval_program,
    eval_sdf_function, eval_sdf_function_args_with_overrides, eval_sdf_function_with_overrides,
    eval_sdf_vec3_function_with_overrides, eval_sdf_zero_arg_function,
    eval_sdf_zero_arg_function_with_overrides, eval_top_level_function,
};
pub use materials::{
    BlendedMaterial, BsdfSample as MaterialBsdfSample, ColorPattern, DielectricMaterial,
    LambertMaterial, Material, MaterialBsdf, MaterialKindTag, MaterialParams, MediumParams,
    MetalMaterial, SampleInput as MaterialSampleInput, SubsurfaceParams,
};
pub use parser::{ParseError, parse_program};
pub use render_api::{
    Bsdf, Camera, CameraKind, EnvLight, Integrator, Light, LightSample, MaterialKind,
    MaterialModel, OpenPbrMaterial, PinholeCamera, PointLight, PreviewIntegrator, Ray, Spectrum,
    SphereLight, SurfaceHit, Vec3,
};
pub use renderer::{
    AccelMode, PreviewProgress, RayDebugAov, RayProgress, RaySettings, RenderError, RenderOptions,
    SceneRenderSettings, extract_scene_render_settings, render_depth_png,
    render_depth_png_with_accel, render_preview_progressive_with_accel, render_ray_png_with_accel,
    render_ray_progressive_with_accel,
};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinLibraryCategory {
    Materials,
    Objects,
    Skeletons,
    Scenes,
}

#[derive(Debug, Clone, Copy)]
pub struct BuiltinLibraryItem {
    pub category: BuiltinLibraryCategory,
    pub name: &'static str,
    pub path: &'static str,
    pub description: &'static str,
    pub tags: &'static [&'static str],
    pub source: &'static str,
}

const BUILTIN_LIBRARY: &[BuiltinLibraryItem] = &[
    BuiltinLibraryItem {
        category: BuiltinLibraryCategory::Materials,
        name: "Gold",
        path: "materials/gold.ft",
        description: "Polished gold metal with moderate roughness for reflective showcase materials.",
        tags: &["material", "metal", "gold", "reflective", "warm"],
        source: include_str!("../library/materials/gold.ft"),
    },
    BuiltinLibraryItem {
        category: BuiltinLibraryCategory::Materials,
        name: "Glass",
        path: "materials/glass.ft",
        description: "Clear dielectric glass material for transmissive and refractive surfaces.",
        tags: &[
            "material",
            "glass",
            "dielectric",
            "transmission",
            "refractive",
        ],
        source: include_str!("../library/materials/glass.ft"),
    },
    BuiltinLibraryItem {
        category: BuiltinLibraryCategory::Materials,
        name: "CheckerFloor",
        path: "materials/checker_floor.ft",
        description: "Diffuse procedural checker material for floors, stages, and reference scenes.",
        tags: &["material", "checker", "floor", "diffuse", "procedural"],
        source: include_str!("../library/materials/checker_floor.ft"),
    },
    BuiltinLibraryItem {
        category: BuiltinLibraryCategory::Materials,
        name: "StoneMoss",
        path: "materials/stone_moss.ft",
        description: "Layered wet stone material with darker moss patches and local-space breakup.",
        tags: &["material", "stone", "moss", "wet", "layered", "procedural"],
        source: include_str!("../library/materials/stone_moss.ft"),
    },
    BuiltinLibraryItem {
        category: BuiltinLibraryCategory::Objects,
        name: "SoftBlob",
        path: "objects/soft_blob.ft",
        description: "Custom Forge SDF blob with warped silhouette and conservative bounds helper.",
        tags: &["object", "sdf", "blob", "organic", "procedural"],
        source: include_str!("../library/objects/soft_blob.ft"),
    },
    BuiltinLibraryItem {
        category: BuiltinLibraryCategory::Objects,
        name: "Cupboard",
        path: "objects/cupboard.ft",
        description: "Simple parameterized cupboard shell with a front panel openness control.",
        tags: &["object", "furniture", "cupboard", "storage", "parametric"],
        source: include_str!("../library/objects/cupboard.ft"),
    },
    BuiltinLibraryItem {
        category: BuiltinLibraryCategory::Objects,
        name: "Table",
        path: "objects/table.ft",
        description: "Parameterized table with a rectangular top and four round legs.",
        tags: &["object", "furniture", "table", "surface", "parametric"],
        source: include_str!("../library/objects/table.ft"),
    },
    BuiltinLibraryItem {
        category: BuiltinLibraryCategory::Objects,
        name: "Lamp",
        path: "objects/lamp.ft",
        description: "Parameterized table lamp with separate body, shade, and bulb material slots.",
        tags: &["object", "furniture", "lamp", "lighting", "parametric"],
        source: include_str!("../library/objects/lamp.ft"),
    },
    BuiltinLibraryItem {
        category: BuiltinLibraryCategory::Objects,
        name: "TwistedStatue",
        path: "objects/twisted_statue.ft",
        description: "Procedural twisted shell statue with fine horizontal banding for sculptural accents.",
        tags: &["object", "statue", "sculpture", "procedural", "decor"],
        source: include_str!("../library/objects/twisted_statue.ft"),
    },
    BuiltinLibraryItem {
        category: BuiltinLibraryCategory::Objects,
        name: "RobotSegment",
        path: "objects/robot_segment.ft",
        description: "Reusable rigid robot limb segment modeled in a canonical bind pose.",
        tags: &["object", "robot", "part", "limb", "bindable"],
        source: include_str!("../library/objects/robot_segment.ft"),
    },
    BuiltinLibraryItem {
        category: BuiltinLibraryCategory::Objects,
        name: "RobotTorso",
        path: "objects/robot_torso.ft",
        description: "Reusable robot torso segment modeled in a canonical bind pose.",
        tags: &["object", "robot", "torso", "part", "bindable"],
        source: include_str!("../library/objects/robot_torso.ft"),
    },
    BuiltinLibraryItem {
        category: BuiltinLibraryCategory::Objects,
        name: "RobotHead",
        path: "objects/robot_head.ft",
        description: "Simple rounded robot head for skeleton scenes.",
        tags: &["object", "robot", "head", "part"],
        source: include_str!("../library/objects/robot_head.ft"),
    },
    BuiltinLibraryItem {
        category: BuiltinLibraryCategory::Objects,
        name: "RobotJoint",
        path: "objects/robot_joint.ft",
        description: "Spherical robot joint marker for articulated assemblies.",
        tags: &["object", "robot", "joint", "part"],
        source: include_str!("../library/objects/robot_joint.ft"),
    },
    BuiltinLibraryItem {
        category: BuiltinLibraryCategory::Objects,
        name: "RobotFoot",
        path: "objects/robot_foot.ft",
        description: "Simple forward-offset robot foot block.",
        tags: &["object", "robot", "foot", "part"],
        source: include_str!("../library/objects/robot_foot.ft"),
    },
    BuiltinLibraryItem {
        category: BuiltinLibraryCategory::Objects,
        name: "RobotBody",
        path: "objects/robot_body.ft",
        description: "Semantic assembled robot body driven by a skeleton.",
        tags: &["object", "robot", "body", "skeleton", "semantic"],
        source: include_str!("../library/objects/robot_body.ft"),
    },
    BuiltinLibraryItem {
        category: BuiltinLibraryCategory::Skeletons,
        name: "Robot",
        path: "skeletons/robot.ft",
        description: "Simplified rigid biped robot skeleton with named joints and segment bones.",
        tags: &["skeleton", "robot", "biped", "rig", "semantic"],
        source: include_str!("../library/skeletons/robot.ft"),
    },
    BuiltinLibraryItem {
        category: BuiltinLibraryCategory::Scenes,
        name: "Studio",
        path: "scenes/studio.ft",
        description: "Reusable studio-style scene setup with camera and lighting defaults.",
        tags: &["scene", "studio", "camera", "lights", "starter"],
        source: include_str!("../library/scenes/studio.ft"),
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltinLibraryMetadata {
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub params: Vec<BuiltinLibraryParam>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltinLibraryParam {
    pub name: String,
    pub param_type: String,
    pub description: Option<String>,
    pub default: Option<String>,
    pub min: Option<String>,
    pub max: Option<String>,
}

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
    #[error("missing import '{path}' from {from}")]
    MissingImport { from: PathBuf, path: String },
    #[error("ambiguous built-in import '{name}' matches: {matches}")]
    AmbiguousBuiltinImport { name: String, matches: String },
    #[error("import cycle detected: {0}")]
    ImportCycle(String),
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
    let program = load_program_with_imports(scene_path)?;
    let state = eval_program(&program)?;
    Ok(state)
}

pub fn load_program_with_imports(scene_path: &Path) -> Result<Program, CoreError> {
    let mut loaded = HashSet::new();
    let mut stack = Vec::new();
    let statements = load_program_statements(scene_path, &mut loaded, &mut stack)?;
    Ok(Program { statements })
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ImportKey {
    File(PathBuf),
    Builtin(String),
}

fn load_program_statements(
    scene_path: &Path,
    loaded: &mut HashSet<(ImportKey, Option<String>)>,
    stack: &mut Vec<ImportKey>,
) -> Result<Vec<Statement>, CoreError> {
    let canonical = scene_path.canonicalize().map_err(|source| CoreError::Io {
        path: scene_path.to_path_buf(),
        source,
    })?;
    load_import_source(
        ImportKey::File(canonical.clone()),
        None,
        Some(canonical.parent().unwrap_or(Path::new("."))),
        loaded,
        stack,
    )
}

fn load_import_source(
    key: ImportKey,
    namespace: Option<String>,
    parent_dir: Option<&Path>,
    loaded: &mut HashSet<(ImportKey, Option<String>)>,
    stack: &mut Vec<ImportKey>,
) -> Result<Vec<Statement>, CoreError> {
    if loaded.contains(&(key.clone(), namespace.clone())) {
        return Ok(Vec::new());
    }
    if stack.contains(&key) {
        let mut cycle = stack.iter().map(import_key_display).collect::<Vec<_>>();
        cycle.push(import_key_display(&key));
        return Err(CoreError::ImportCycle(cycle.join(" -> ")));
    }

    stack.push(key.clone());
    let (source, base_dir) = match &key {
        ImportKey::File(path) => (
            fs::read_to_string(path).map_err(|source| CoreError::Io {
                path: path.clone(),
                source,
            })?,
            Some(path.parent().unwrap_or(Path::new(".")).to_path_buf()),
        ),
        ImportKey::Builtin(path) => (builtin_library_source(path).unwrap_or("").to_string(), None),
    };
    if matches!(&key, ImportKey::Builtin(path) if builtin_library_source(path).is_none()) {
        let from = parent_dir.unwrap_or(Path::new(".")).to_path_buf();
        return Err(CoreError::MissingImport {
            from,
            path: match &key {
                ImportKey::Builtin(path) => path.clone(),
                ImportKey::File(path) => path.display().to_string(),
            },
        });
    }

    let program = parse_program(&source)?;
    let export_names = collect_export_names(&program.statements);
    let mut statements = Vec::new();
    for stmt in program.statements {
        match stmt {
            Statement::Import { path, alias } => {
                let resolved = resolve_import(&path, base_dir.as_deref())?;
                statements.extend(load_import_source(
                    resolved,
                    alias,
                    base_dir.as_deref(),
                    loaded,
                    stack,
                )?);
            }
            Statement::Export(_) => {}
            other => statements.push(other),
        }
    }
    if let Some(export_names) = export_names {
        statements = filter_exported_statements(statements, &export_names);
    }
    stack.pop();
    if let Some(alias) = namespace.clone() {
        statements = namespace_statements(statements, &alias);
    }
    loaded.insert((key, namespace));
    Ok(statements)
}

fn resolve_import(path: &str, base_dir: Option<&Path>) -> Result<ImportKey, CoreError> {
    if path.starts_with("./") || path.starts_with("../") {
        let from = base_dir.unwrap_or(Path::new(".")).to_path_buf();
        let resolved = from.join(path);
        if !resolved.exists() {
            return Err(CoreError::MissingImport {
                from,
                path: path.to_string(),
            });
        }
        let canonical = resolved.canonicalize().map_err(|source| CoreError::Io {
            path: resolved,
            source,
        })?;
        return Ok(ImportKey::File(canonical));
    }

    if builtin_library_source(path).is_some() {
        return Ok(ImportKey::Builtin(path.to_string()));
    }

    if let Some(item) = resolve_builtin_library_name(path)? {
        return Ok(ImportKey::Builtin(item.path.to_string()));
    }

    let from = base_dir.unwrap_or(Path::new(".")).to_path_buf();
    Err(CoreError::MissingImport {
        from,
        path: path.to_string(),
    })
}

fn import_key_display(key: &ImportKey) -> String {
    match key {
        ImportKey::File(path) => path.display().to_string(),
        ImportKey::Builtin(path) => format!("builtin:{path}"),
    }
}

fn builtin_library_source(path: &str) -> Option<&'static str> {
    builtin_library_item_by_path(path).map(|item| item.source)
}

fn builtin_library_item_by_path(path: &str) -> Option<&'static BuiltinLibraryItem> {
    BUILTIN_LIBRARY.iter().find(|item| item.path == path)
}

fn resolve_builtin_library_name(
    name: &str,
) -> Result<Option<&'static BuiltinLibraryItem>, CoreError> {
    let matches = BUILTIN_LIBRARY
        .iter()
        .filter(|item| item.name == name)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => Ok(None),
        [item] => Ok(Some(*item)),
        many => Err(CoreError::AmbiguousBuiltinImport {
            name: name.to_string(),
            matches: many
                .iter()
                .map(|item| item.path)
                .collect::<Vec<_>>()
                .join(", "),
        }),
    }
}

pub fn builtin_library_items(category: Option<BuiltinLibraryCategory>) -> Vec<BuiltinLibraryItem> {
    BUILTIN_LIBRARY
        .iter()
        .copied()
        .filter(|item| category.is_none_or(|cat| item.category == cat))
        .collect()
}

pub fn builtin_library_item_metadata(item: &BuiltinLibraryItem) -> BuiltinLibraryMetadata {
    extract_builtin_metadata(item).unwrap_or_else(|| BuiltinLibraryMetadata {
        name: item.name.to_string(),
        description: item.description.to_string(),
        tags: item.tags.iter().map(|tag| (*tag).to_string()).collect(),
        params: Vec::new(),
    })
}

fn extract_builtin_metadata(item: &BuiltinLibraryItem) -> Option<BuiltinLibraryMetadata> {
    let program = parse_program(item.source).ok()?;
    for stmt in program.statements {
        match stmt {
            Statement::MaterialDef(def) if def.name == item.name => {
                return Some(metadata_from_pairs(
                    item.name,
                    item.description,
                    item.tags,
                    &def.metadata,
                ));
            }
            Statement::SdfDef(def) if def.name == item.name => {
                return Some(metadata_from_pairs(
                    item.name,
                    item.description,
                    item.tags,
                    &def.metadata,
                ));
            }
            Statement::SkeletonDef(def) if def.name == item.name => {
                return Some(metadata_from_pairs(
                    item.name,
                    item.description,
                    item.tags,
                    &def.metadata,
                ));
            }
            Statement::EnvironmentDef(def) if def.name == item.name => {
                return Some(metadata_from_pairs(
                    item.name,
                    item.description,
                    item.tags,
                    &def.metadata,
                ));
            }
            _ => {}
        }
    }
    None
}

fn metadata_from_pairs(
    fallback_name: &str,
    fallback_description: &str,
    fallback_tags: &[&str],
    metadata: &[(String, Expr)],
) -> BuiltinLibraryMetadata {
    let mut name = fallback_name.to_string();
    let mut description = fallback_description.to_string();
    let mut tags = fallback_tags
        .iter()
        .map(|tag| (*tag).to_string())
        .collect::<Vec<_>>();
    let mut params = Vec::new();

    for (key, expr) in metadata {
        match key.as_str() {
            "name" => {
                if let Some(value) = metadata_string(expr) {
                    name = value;
                }
            }
            "description" => {
                if let Some(value) = metadata_string(expr) {
                    description = value;
                }
            }
            "tags" => {
                if let Some(values) = metadata_string_array(expr) {
                    tags = values;
                }
            }
            "params" => {
                if let Some(values) = metadata_params(expr) {
                    params = values;
                }
            }
            _ => {}
        }
    }

    BuiltinLibraryMetadata {
        name,
        description,
        tags,
        params,
    }
}

fn metadata_string(expr: &Expr) -> Option<String> {
    match expr {
        Expr::String(value) => Some(value.clone()),
        _ => None,
    }
}

fn metadata_string_array(expr: &Expr) -> Option<Vec<String>> {
    match expr {
        Expr::Array(items) => items.iter().map(metadata_string).collect(),
        _ => None,
    }
}

fn metadata_params(expr: &Expr) -> Option<Vec<BuiltinLibraryParam>> {
    let Expr::Array(items) = expr else {
        return None;
    };
    items.iter().map(metadata_param).collect()
}

fn metadata_param(expr: &Expr) -> Option<BuiltinLibraryParam> {
    let Expr::ObjectLiteral { fields, .. } = expr else {
        return None;
    };
    let mut name = None;
    let mut param_type = None;
    let mut description = None;
    let mut default = None;
    let mut min = None;
    let mut max = None;

    for (key, value) in fields {
        match key.as_str() {
            "name" => name = metadata_string(value),
            "type" => param_type = metadata_string(value),
            "description" => description = metadata_string(value),
            "default" => default = metadata_scalar_string(value),
            "min" => min = metadata_scalar_string(value),
            "max" => max = metadata_scalar_string(value),
            _ => {}
        }
    }

    Some(BuiltinLibraryParam {
        name: name?,
        param_type: param_type?,
        description,
        default,
        min,
        max,
    })
}

fn metadata_scalar_string(expr: &Expr) -> Option<String> {
    match expr {
        Expr::String(value) => Some(value.clone()),
        Expr::Number(value) => Some(value.to_string()),
        Expr::Ident(value) => Some(value.clone()),
        _ => None,
    }
}

fn namespace_statements(statements: Vec<Statement>, alias: &str) -> Vec<Statement> {
    let mut top_level_names = HashSet::new();
    for stmt in &statements {
        match stmt {
            Statement::Binding { name, .. } => {
                top_level_names.insert(name.clone());
            }
            Statement::FunctionDef(def) => {
                top_level_names.insert(def.name.clone());
            }
            Statement::MaterialDef(def) => {
                top_level_names.insert(def.name.clone());
            }
            Statement::SdfDef(def) => {
                top_level_names.insert(def.name.clone());
            }
            Statement::SkeletonDef(def) => {
                top_level_names.insert(def.name.clone());
            }
            Statement::EnvironmentDef(def) => {
                top_level_names.insert(def.name.clone());
            }
            Statement::Assign { .. } | Statement::Import { .. } | Statement::Export(_) => {}
        }
    }

    statements
        .into_iter()
        .map(|stmt| namespace_statement(stmt, alias, &top_level_names))
        .collect()
}

fn namespace_statement(stmt: Statement, alias: &str, names: &HashSet<String>) -> Statement {
    match stmt {
        Statement::Binding {
            name,
            mutable,
            expr,
        } => Statement::Binding {
            name: qualify_name(alias, &name),
            mutable,
            expr: namespace_expr(expr, alias, names, &HashSet::new()),
        },
        Statement::Assign { path, expr } => Statement::Assign {
            path: namespace_path(path, alias, names),
            expr: namespace_expr(expr, alias, names, &HashSet::new()),
        },
        Statement::FunctionDef(mut def) => {
            let mut fn_scope = HashSet::new();
            for param in &def.params {
                fn_scope.insert(param.clone());
            }
            let mut rewritten_body = Vec::with_capacity(def.body.len());
            for stmt in def.body {
                match stmt {
                    ast::MaterialFunctionStatement::Binding { name, expr } => {
                        let expr = namespace_expr(expr, alias, names, &fn_scope);
                        fn_scope.insert(name.clone());
                        rewritten_body.push(ast::MaterialFunctionStatement::Binding { name, expr });
                    }
                    ast::MaterialFunctionStatement::Return { expr } => {
                        rewritten_body.push(ast::MaterialFunctionStatement::Return {
                            expr: namespace_expr(expr, alias, names, &fn_scope),
                        });
                    }
                }
            }
            def.name = qualify_name(alias, &def.name);
            def.body = rewritten_body;
            Statement::FunctionDef(def)
        }
        Statement::MaterialDef(mut def) => {
            let mut scope = HashSet::new();
            for stmt in &def.statements {
                if let ast::MaterialStatement::Binding { name, .. } = stmt {
                    scope.insert(name.clone());
                }
            }
            def.name = qualify_name(alias, &def.name);
            def.metadata = def
                .metadata
                .into_iter()
                .map(|(name, expr)| (name, namespace_expr(expr, alias, names, &scope)))
                .collect();
            def.statements = def
                .statements
                .into_iter()
                .map(|stmt| match stmt {
                    ast::MaterialStatement::Binding { name, expr } => {
                        ast::MaterialStatement::Binding {
                            name,
                            expr: namespace_expr(expr, alias, names, &scope),
                        }
                    }
                    ast::MaterialStatement::Property { name, expr } => {
                        ast::MaterialStatement::Property {
                            name,
                            expr: namespace_expr(expr, alias, names, &scope),
                        }
                    }
                    ast::MaterialStatement::Function { name, params, body } => {
                        let mut fn_scope = scope.clone();
                        for param in &params {
                            fn_scope.insert(param.clone());
                        }
                        let mut rewritten_body = Vec::with_capacity(body.len());
                        for stmt in body {
                            match stmt {
                                ast::MaterialFunctionStatement::Binding { name, expr } => {
                                    let expr = namespace_expr(expr, alias, names, &fn_scope);
                                    fn_scope.insert(name.clone());
                                    rewritten_body.push(ast::MaterialFunctionStatement::Binding {
                                        name,
                                        expr,
                                    });
                                }
                                ast::MaterialFunctionStatement::Return { expr } => {
                                    rewritten_body.push(ast::MaterialFunctionStatement::Return {
                                        expr: namespace_expr(expr, alias, names, &fn_scope),
                                    });
                                }
                            }
                        }
                        let body = rewritten_body;
                        ast::MaterialStatement::Function { name, params, body }
                    }
                })
                .collect();
            Statement::MaterialDef(def)
        }
        Statement::SdfDef(mut def) => {
            let mut scope = HashSet::new();
            for stmt in &def.statements {
                if let ast::SdfStatement::Binding { name, .. } = stmt {
                    scope.insert(name.clone());
                }
            }
            def.name = qualify_name(alias, &def.name);
            def.metadata = def
                .metadata
                .into_iter()
                .map(|(name, expr)| (name, namespace_expr(expr, alias, names, &scope)))
                .collect();
            def.statements = def
                .statements
                .into_iter()
                .map(|stmt| match stmt {
                    ast::SdfStatement::Binding { name, expr } => ast::SdfStatement::Binding {
                        name,
                        expr: namespace_expr(expr, alias, names, &scope),
                    },
                    ast::SdfStatement::Function { name, params, body } => {
                        let mut fn_scope = scope.clone();
                        for param in &params {
                            fn_scope.insert(param.clone());
                        }
                        let mut rewritten_body = Vec::with_capacity(body.len());
                        for stmt in body {
                            match stmt {
                                ast::MaterialFunctionStatement::Binding { name, expr } => {
                                    let expr = namespace_expr(expr, alias, names, &fn_scope);
                                    fn_scope.insert(name.clone());
                                    rewritten_body.push(ast::MaterialFunctionStatement::Binding {
                                        name,
                                        expr,
                                    });
                                }
                                ast::MaterialFunctionStatement::Return { expr } => {
                                    rewritten_body.push(ast::MaterialFunctionStatement::Return {
                                        expr: namespace_expr(expr, alias, names, &fn_scope),
                                    });
                                }
                            }
                        }
                        let body = rewritten_body;
                        ast::SdfStatement::Function { name, params, body }
                    }
                })
                .collect();
            Statement::SdfDef(def)
        }
        Statement::EnvironmentDef(mut def) => {
            let mut scope = HashSet::new();
            for stmt in &def.statements {
                if let ast::MaterialStatement::Binding { name, .. } = stmt {
                    scope.insert(name.clone());
                }
            }
            def.name = qualify_name(alias, &def.name);
            def.metadata = def
                .metadata
                .into_iter()
                .map(|(name, expr)| (name, namespace_expr(expr, alias, names, &scope)))
                .collect();
            def.statements = def
                .statements
                .into_iter()
                .map(|stmt| match stmt {
                    ast::MaterialStatement::Binding { name, expr } => {
                        ast::MaterialStatement::Binding {
                            name,
                            expr: namespace_expr(expr, alias, names, &scope),
                        }
                    }
                    ast::MaterialStatement::Property { name, expr } => {
                        ast::MaterialStatement::Property {
                            name,
                            expr: namespace_expr(expr, alias, names, &scope),
                        }
                    }
                    ast::MaterialStatement::Function { name, params, body } => {
                        let mut fn_scope = scope.clone();
                        for param in &params {
                            fn_scope.insert(param.clone());
                        }
                        let mut rewritten_body = Vec::with_capacity(body.len());
                        for stmt in body {
                            match stmt {
                                ast::MaterialFunctionStatement::Binding { name, expr } => {
                                    let expr = namespace_expr(expr, alias, names, &fn_scope);
                                    fn_scope.insert(name.clone());
                                    rewritten_body.push(ast::MaterialFunctionStatement::Binding {
                                        name,
                                        expr,
                                    });
                                }
                                ast::MaterialFunctionStatement::Return { expr } => {
                                    rewritten_body.push(ast::MaterialFunctionStatement::Return {
                                        expr: namespace_expr(expr, alias, names, &fn_scope),
                                    });
                                }
                            }
                        }
                        let body = rewritten_body;
                        ast::MaterialStatement::Function { name, params, body }
                    }
                })
                .collect();
            Statement::EnvironmentDef(def)
        }
        Statement::SkeletonDef(mut def) => {
            let mut scope = HashSet::new();
            for stmt in &def.statements {
                if let ast::SkeletonStatement::Binding { name, .. } = stmt {
                    scope.insert(name.clone());
                }
                if let ast::SkeletonStatement::Joint { name, .. } = stmt {
                    scope.insert(name.clone());
                }
            }
            def.name = qualify_name(alias, &def.name);
            def.metadata = def
                .metadata
                .into_iter()
                .map(|(name, expr)| (name, namespace_expr(expr, alias, names, &scope)))
                .collect();
            def.statements = def
                .statements
                .into_iter()
                .map(|stmt| match stmt {
                    ast::SkeletonStatement::Binding { name, expr } => {
                        ast::SkeletonStatement::Binding {
                            name,
                            expr: namespace_expr(expr, alias, names, &scope),
                        }
                    }
                    ast::SkeletonStatement::Joint { name, expr } => ast::SkeletonStatement::Joint {
                        name,
                        expr: namespace_expr(expr, alias, names, &scope),
                    },
                    ast::SkeletonStatement::Bone { name, start, end } => {
                        ast::SkeletonStatement::Bone { name, start, end }
                    }
                    ast::SkeletonStatement::Chain {
                        name,
                        start,
                        mid,
                        end,
                    } => ast::SkeletonStatement::Chain {
                        name,
                        start,
                        mid,
                        end,
                    },
                })
                .collect();
            Statement::SkeletonDef(def)
        }
        Statement::Import { path, alias } => Statement::Import { path, alias },
        Statement::Export(names) => Statement::Export(names),
    }
}

fn collect_export_names(statements: &[Statement]) -> Option<HashSet<String>> {
    let mut names = HashSet::new();
    let mut found = false;
    for stmt in statements {
        if let Statement::Export(exported) = stmt {
            found = true;
            names.extend(exported.iter().cloned());
        }
    }
    found.then_some(names)
}

fn filter_exported_statements(
    statements: Vec<Statement>,
    export_names: &HashSet<String>,
) -> Vec<Statement> {
    let mut by_name = HashMap::new();
    for stmt in &statements {
        match stmt {
            Statement::Binding { name, .. } => {
                by_name.insert(name.clone(), stmt.clone());
            }
            Statement::FunctionDef(def) => {
                by_name.insert(def.name.clone(), stmt.clone());
            }
            Statement::MaterialDef(def) => {
                by_name.insert(def.name.clone(), stmt.clone());
            }
            Statement::SdfDef(def) => {
                by_name.insert(def.name.clone(), stmt.clone());
            }
            Statement::SkeletonDef(def) => {
                by_name.insert(def.name.clone(), stmt.clone());
            }
            Statement::EnvironmentDef(def) => {
                by_name.insert(def.name.clone(), stmt.clone());
            }
            Statement::Assign { .. } | Statement::Import { .. } | Statement::Export(_) => {}
        }
    }

    let mut keep = HashSet::new();
    let mut stack = export_names.iter().cloned().collect::<Vec<_>>();
    while let Some(name) = stack.pop() {
        if !keep.insert(name.clone()) {
            continue;
        }
        let Some(stmt) = by_name.get(&name) else {
            continue;
        };
        for dep in statement_dependencies(stmt) {
            if by_name.contains_key(&dep) {
                stack.push(dep);
            }
        }
    }

    statements
        .into_iter()
        .filter(|stmt| match stmt {
            Statement::Binding { name, .. } => keep.contains(name),
            Statement::FunctionDef(def) => keep.contains(&def.name),
            Statement::MaterialDef(def) => keep.contains(&def.name),
            Statement::SdfDef(def) => keep.contains(&def.name),
            Statement::SkeletonDef(def) => keep.contains(&def.name),
            Statement::EnvironmentDef(def) => keep.contains(&def.name),
            Statement::Assign { path, .. } => path.first().is_some_and(|name| keep.contains(name)),
            Statement::Import { .. } | Statement::Export(_) => false,
        })
        .collect()
}

fn statement_dependencies(stmt: &Statement) -> HashSet<String> {
    match stmt {
        Statement::Binding { expr, .. } => expr_dependencies(expr, &HashSet::new()),
        Statement::Assign { expr, .. } => expr_dependencies(expr, &HashSet::new()),
        Statement::FunctionDef(def) => {
            let mut deps = HashSet::new();
            let mut scope = HashSet::new();
            for param in &def.params {
                scope.insert(param.clone());
            }
            for stmt in &def.body {
                match stmt {
                    ast::MaterialFunctionStatement::Binding { name, expr } => {
                        deps.extend(expr_dependencies(expr, &scope));
                        scope.insert(name.clone());
                    }
                    ast::MaterialFunctionStatement::Return { expr } => {
                        deps.extend(expr_dependencies(expr, &scope));
                    }
                }
            }
            deps
        }
        Statement::MaterialDef(def) => {
            let mut deps = HashSet::new();
            let mut scope = HashSet::new();
            for stmt in &def.statements {
                if let ast::MaterialStatement::Binding { name, .. } = stmt {
                    scope.insert(name.clone());
                }
            }
            for (_, expr) in &def.metadata {
                deps.extend(expr_dependencies(expr, &scope));
            }
            for stmt in &def.statements {
                match stmt {
                    ast::MaterialStatement::Binding { expr, .. }
                    | ast::MaterialStatement::Property { expr, .. } => {
                        deps.extend(expr_dependencies(expr, &scope));
                    }
                    ast::MaterialStatement::Function { params, body, .. } => {
                        let mut fn_scope = scope.clone();
                        for param in params {
                            fn_scope.insert(param.clone());
                        }
                        for stmt in body {
                            match stmt {
                                ast::MaterialFunctionStatement::Binding { name, expr } => {
                                    deps.extend(expr_dependencies(expr, &fn_scope));
                                    fn_scope.insert(name.clone());
                                }
                                ast::MaterialFunctionStatement::Return { expr } => {
                                    deps.extend(expr_dependencies(expr, &fn_scope));
                                }
                            }
                        }
                    }
                }
            }
            deps
        }
        Statement::SdfDef(def) => {
            let mut deps = HashSet::new();
            let mut scope = HashSet::new();
            for stmt in &def.statements {
                if let ast::SdfStatement::Binding { name, .. } = stmt {
                    scope.insert(name.clone());
                }
            }
            for (_, expr) in &def.metadata {
                deps.extend(expr_dependencies(expr, &scope));
            }
            for stmt in &def.statements {
                match stmt {
                    ast::SdfStatement::Binding { expr, .. } => {
                        deps.extend(expr_dependencies(expr, &scope));
                    }
                    ast::SdfStatement::Function { params, body, .. } => {
                        let mut fn_scope = scope.clone();
                        for param in params {
                            fn_scope.insert(param.clone());
                        }
                        for stmt in body {
                            match stmt {
                                ast::MaterialFunctionStatement::Binding { name, expr } => {
                                    deps.extend(expr_dependencies(expr, &fn_scope));
                                    fn_scope.insert(name.clone());
                                }
                                ast::MaterialFunctionStatement::Return { expr } => {
                                    deps.extend(expr_dependencies(expr, &fn_scope));
                                }
                            }
                        }
                    }
                }
            }
            deps
        }
        Statement::SkeletonDef(def) => {
            let mut deps = HashSet::new();
            let mut scope = HashSet::new();
            for stmt in &def.statements {
                match stmt {
                    ast::SkeletonStatement::Binding { name, .. }
                    | ast::SkeletonStatement::Joint { name, .. } => {
                        scope.insert(name.clone());
                    }
                    ast::SkeletonStatement::Bone { .. } | ast::SkeletonStatement::Chain { .. } => {}
                }
            }
            for (_, expr) in &def.metadata {
                deps.extend(expr_dependencies(expr, &scope));
            }
            for stmt in &def.statements {
                match stmt {
                    ast::SkeletonStatement::Binding { expr, .. }
                    | ast::SkeletonStatement::Joint { expr, .. } => {
                        deps.extend(expr_dependencies(expr, &scope));
                    }
                    ast::SkeletonStatement::Bone { start, end, .. } => {
                        if !scope.contains(start) {
                            deps.insert(start.clone());
                        }
                        if !scope.contains(end) {
                            deps.insert(end.clone());
                        }
                    }
                    ast::SkeletonStatement::Chain {
                        start, mid, end, ..
                    } => {
                        if !scope.contains(start) {
                            deps.insert(start.clone());
                        }
                        if !scope.contains(mid) {
                            deps.insert(mid.clone());
                        }
                        if !scope.contains(end) {
                            deps.insert(end.clone());
                        }
                    }
                }
            }
            deps
        }
        Statement::EnvironmentDef(def) => {
            let mut deps = HashSet::new();
            let mut scope = HashSet::new();
            for stmt in &def.statements {
                if let ast::MaterialStatement::Binding { name, .. } = stmt {
                    scope.insert(name.clone());
                }
            }
            for (_, expr) in &def.metadata {
                deps.extend(expr_dependencies(expr, &scope));
            }
            for stmt in &def.statements {
                match stmt {
                    ast::MaterialStatement::Binding { expr, .. }
                    | ast::MaterialStatement::Property { expr, .. } => {
                        deps.extend(expr_dependencies(expr, &scope));
                    }
                    ast::MaterialStatement::Function { params, body, .. } => {
                        let mut fn_scope = scope.clone();
                        for param in params {
                            fn_scope.insert(param.clone());
                        }
                        for stmt in body {
                            match stmt {
                                ast::MaterialFunctionStatement::Binding { name, expr } => {
                                    deps.extend(expr_dependencies(expr, &fn_scope));
                                    fn_scope.insert(name.clone());
                                }
                                ast::MaterialFunctionStatement::Return { expr } => {
                                    deps.extend(expr_dependencies(expr, &fn_scope));
                                }
                            }
                        }
                    }
                }
            }
            deps
        }
        Statement::Import { .. } | Statement::Export(_) => HashSet::new(),
    }
}

fn expr_dependencies(expr: &Expr, local_scope: &HashSet<String>) -> HashSet<String> {
    let mut deps = HashSet::new();
    match expr {
        Expr::String(_) => {}
        Expr::Array(items) => {
            for item in items {
                deps.extend(expr_dependencies(item, local_scope));
            }
        }
        Expr::Ident(name) => {
            if !local_scope.contains(name) {
                deps.insert(name.clone());
            }
        }
        Expr::ObjectLiteral { type_name, fields } => {
            deps.insert(type_name.clone());
            for (_, expr) in fields {
                deps.extend(expr_dependencies(expr, local_scope));
            }
        }
        Expr::Binary { lhs, rhs, .. } => {
            deps.extend(expr_dependencies(lhs, local_scope));
            deps.extend(expr_dependencies(rhs, local_scope));
        }
        Expr::Member { .. } => {
            if let Some(name) = flatten_expr_name(expr)
                && !local_scope.contains(name.split('.').next().unwrap_or_default())
            {
                deps.insert(name);
            }
        }
        Expr::Call { callee, args } => {
            deps.extend(expr_dependencies(callee, local_scope));
            for arg in args {
                deps.extend(expr_dependencies(arg, local_scope));
            }
        }
        Expr::Unary { expr, .. } => {
            deps.extend(expr_dependencies(expr, local_scope));
        }
        Expr::FunctionLiteral { .. } => {}
        Expr::Number(_) => {}
    }
    deps
}

fn flatten_expr_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Ident(name) => Some(name.clone()),
        Expr::Member { target, field } => Some(format!("{}.{}", flatten_expr_name(target)?, field)),
        _ => None,
    }
}

fn namespace_expr(
    expr: Expr,
    alias: &str,
    top_level_names: &HashSet<String>,
    local_scope: &HashSet<String>,
) -> Expr {
    match expr {
        Expr::String(_) => expr,
        Expr::Array(items) => Expr::Array(
            items
                .into_iter()
                .map(|item| namespace_expr(item, alias, top_level_names, local_scope))
                .collect(),
        ),
        Expr::Ident(name) => {
            if top_level_names.contains(&name) && !local_scope.contains(&name) {
                Expr::Ident(qualify_name(alias, &name))
            } else {
                Expr::Ident(name)
            }
        }
        Expr::ObjectLiteral { type_name, fields } => {
            let type_name = if top_level_names.contains(&type_name) {
                qualify_name(alias, &type_name)
            } else {
                type_name
            };
            Expr::ObjectLiteral {
                type_name,
                fields: fields
                    .into_iter()
                    .map(|(name, expr)| {
                        (
                            name,
                            namespace_expr(expr, alias, top_level_names, local_scope),
                        )
                    })
                    .collect(),
            }
        }
        Expr::Binary { lhs, op, rhs } => Expr::Binary {
            lhs: Box::new(namespace_expr(*lhs, alias, top_level_names, local_scope)),
            op,
            rhs: Box::new(namespace_expr(*rhs, alias, top_level_names, local_scope)),
        },
        Expr::Member { target, field } => Expr::Member {
            target: Box::new(namespace_expr(*target, alias, top_level_names, local_scope)),
            field,
        },
        Expr::Call { callee, args } => Expr::Call {
            callee: Box::new(namespace_expr(*callee, alias, top_level_names, local_scope)),
            args: args
                .into_iter()
                .map(|arg| namespace_expr(arg, alias, top_level_names, local_scope))
                .collect(),
        },
        Expr::Unary { op, expr } => Expr::Unary {
            op,
            expr: Box::new(namespace_expr(*expr, alias, top_level_names, local_scope)),
        },
        Expr::FunctionLiteral { params, body } => Expr::FunctionLiteral { params, body },
        Expr::Number(_) => expr,
    }
}

fn namespace_path(path: Vec<String>, alias: &str, names: &HashSet<String>) -> Vec<String> {
    match path.split_first() {
        Some((head, tail)) if names.contains(head) => {
            let mut out = vec![qualify_name(alias, head)];
            out.extend(tail.iter().cloned());
            out
        }
        _ => path,
    }
}

fn qualify_name(alias: &str, name: &str) -> String {
    format!("{alias}.{name}")
}

#[cfg(test)]
mod tests {
    use super::{
        CoreError, ObjectValue, Value, eval_environment_function,
        eval_material_function_with_overrides, eval_material_properties_with_overrides,
        eval_program, eval_sdf_function, eval_sdf_function_args_with_overrides,
        eval_sdf_function_with_overrides, eval_sdf_vec3_function_with_overrides,
        eval_sdf_zero_arg_function, eval_sdf_zero_arg_function_with_overrides,
        eval_top_level_function, load_and_eval_scene, load_program_with_imports, parse_program,
    };
    use std::{
        collections::HashMap,
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

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
    fn supports_primitive_round_fields() {
        let source = r#"
            let b = Box {
              size: vec3(1.0, 2.0, 3.0),
              round: 0.1
            };
        "#;
        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");
        let value = &state.bindings.get("b").expect("b binding").value;
        let Value::Object(obj) = value else {
            panic!("b should be an object");
        };
        assert_eq!(obj.type_name.as_deref(), Some("Box"));
        assert_eq!(obj.fields.get("round"), Some(&Value::Number(0.1)));
    }

    #[test]
    fn supports_intersection_and_boolean_variant_calls() {
        let source = r#"
            var a = Sphere{};
            var b = Box { size: vec3(1.0) };
            let i = a & b;
            let u = a.union_round(b, 0.2);
            let d = a.diff_stairs(b, 0.3, 5.0);
        "#;
        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");

        let Value::Object(i_obj) = &state.bindings.get("i").expect("i binding").value else {
            panic!("i should be an object");
        };
        assert_eq!(i_obj.type_name.as_deref(), Some("intersect"));

        let Value::Object(u_obj) = &state.bindings.get("u").expect("u binding").value else {
            panic!("u should be an object");
        };
        assert_eq!(u_obj.type_name.as_deref(), Some("union_round"));
        assert_eq!(u_obj.fields.get("r"), Some(&Value::Number(0.2)));

        let Value::Object(d_obj) = &state.bindings.get("d").expect("d binding").value else {
            panic!("d should be an object");
        };
        assert_eq!(d_obj.type_name.as_deref(), Some("diff_stairs"));
        assert_eq!(d_obj.fields.get("n"), Some(&Value::Number(5.0)));
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

    #[test]
    fn supports_semantic_part_material_assignments() {
        let source = r#"
            var table = Table{};
            table.legs.material = Metal { color: #222222, roughness: 0.2 };
            table.top.material = Lambert { color: #f0f0f2 };

            var lamp = Lamp{};
            lamp.bulb.material = Lambert {
              color: #fff3dd,
              emission_color: #fff1d6,
              emission_strength: 5.0
            };
        "#;
        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");

        let Value::Object(table) = &state.bindings.get("table").expect("table binding").value
        else {
            panic!("table should be an object");
        };
        assert!(table.fields.contains_key("leg_material"));
        assert!(table.fields.contains_key("top_material"));

        let Value::Object(lamp) = &state.bindings.get("lamp").expect("lamp binding").value else {
            panic!("lamp should be an object");
        };
        assert!(lamp.fields.contains_key("bulb_material"));
    }

    #[test]
    fn supports_layout_against_semantic_part_proxies() {
        let source = r#"
            import "Table";

            var table = Table {
              width: 1.6,
              depth: 0.8,
              height: 0.75,
              top_thickness: 0.1
            };

            var vase = Sphere { radius: 0.2 }
              .attach(table.top, Top);
        "#;
        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");

        let Value::Object(vase) = &state.bindings.get("vase").expect("vase binding").value else {
            panic!("vase should be an object");
        };
        let Value::Object(vase_pos) = vase.fields.get("pos").expect("vase pos") else {
            panic!("vase pos should be an object");
        };
        assert_eq!(vase_pos.fields.get("y"), Some(&Value::Number(0.575)));
    }

    #[test]
    fn supports_layout_attach_align_and_offset_calls() {
        let source = r#"
            let floor = Box { size: vec3(10.0, 0.5, 10.0) };
            var placed = Sphere { radius: 1.0 }
              .attach(floor, Top)
              .align_x(floor, Center)
              .align_z(floor, Center)
              .offset_x(0.75);
        "#;
        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");
        let value = &state.bindings.get("placed").expect("placed binding").value;
        let Value::Object(obj) = value else {
            panic!("placed should be an object");
        };
        let Value::Object(pos) = obj.fields.get("pos").expect("pos object") else {
            panic!("pos should be an object");
        };
        assert_eq!(pos.fields.get("x"), Some(&Value::Number(0.75)));
        assert_eq!(pos.fields.get("y"), Some(&Value::Number(1.25)));
        assert_eq!(pos.fields.get("z"), Some(&Value::Number(0.0)));
    }

    #[test]
    fn supports_layout_relative_and_rotate_calls() {
        let source = r#"
            let anchor = Sphere { radius: 0.5 };
            var cube = Box { size: vec3(1.0) }
              .right_of(anchor, 0.25)
              .rotate_z(15.0);
        "#;
        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");
        let value = &state.bindings.get("cube").expect("cube binding").value;
        let Value::Object(obj) = value else {
            panic!("cube should be an object");
        };
        let Value::Object(pos) = obj.fields.get("pos").expect("pos object") else {
            panic!("pos should be an object");
        };
        let Value::Object(rot) = obj.fields.get("rot").expect("rot object") else {
            panic!("rot should be an object");
        };
        assert_eq!(pos.fields.get("x"), Some(&Value::Number(1.25)));
        assert_eq!(rot.fields.get("z"), Some(&Value::Number(15.0)));
    }

    #[test]
    fn supports_layout_face_to_calls() {
        let source = r#"
            var target = Sphere { radius: 0.5 };
            target.pos.x = 1.0;
            target.pos.y = 1.0;

            var box = Box { size: vec3(1.0) }
              .face_to(target);
        "#;
        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");
        let value = &state.bindings.get("box").expect("box binding").value;
        let Value::Object(obj) = value else {
            panic!("box should be an object");
        };
        let Value::Object(rot) = obj.fields.get("rot").expect("rot object") else {
            panic!("rot should be an object");
        };
        assert_eq!(rot.fields.get("x"), Some(&Value::Number(-45.0)));
        assert_eq!(rot.fields.get("y"), Some(&Value::Number(90.0)));
    }

    #[test]
    fn supports_domain_helper_member_operators() {
        let source = r#"
            let sphere = Sphere { radius: 0.5 };
            let mirrored = sphere.mirror_x();
            let repeated = sphere.repeat_x(1.5, 3.0);
            let sliced = sphere.slice_y(-0.25, 0.25);
        "#;
        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");

        let Value::Object(mirrored) = &state.bindings.get("mirrored").expect("mirrored").value
        else {
            panic!("mirrored should be an object");
        };
        assert_eq!(mirrored.type_name.as_deref(), Some("mirror_x"));

        let Value::Object(repeated) = &state.bindings.get("repeated").expect("repeated").value
        else {
            panic!("repeated should be an object");
        };
        assert_eq!(repeated.type_name.as_deref(), Some("repeat_x"));

        let Value::Object(sliced) = &state.bindings.get("sliced").expect("sliced").value else {
            panic!("sliced should be an object");
        };
        assert_eq!(sliced.type_name.as_deref(), Some("slice_y"));
    }

    #[test]
    fn supports_layout_anchor_offsets_inside_attach_and_align_calls() {
        let source = r#"
            let floor = Box { size: vec3(10.0, 0.5, 10.0) };
            var placed = Sphere { radius: 1.0 }
              .attach(floor, Top + 0.1)
              .align_z(floor, Center - 0.4);
        "#;
        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");
        let value = &state.bindings.get("placed").expect("placed binding").value;
        let Value::Object(obj) = value else {
            panic!("placed should be an object");
        };
        let Value::Object(pos) = obj.fields.get("pos").expect("pos object") else {
            panic!("pos should be an object");
        };
        assert_eq!(pos.fields.get("y"), Some(&Value::Number(1.35)));
        assert_eq!(pos.fields.get("z"), Some(&Value::Number(-0.4)));
    }

    #[test]
    fn supports_layout_corner_anchors() {
        let source = r#"
            let room = Box { size: vec3(10.0, 4.0, 8.0) };
            var cupboard = Box { size: vec3(2.0, 3.0, 1.5) }
              .attach(room, BackRightCorner);
        "#;
        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");
        let value = &state
            .bindings
            .get("cupboard")
            .expect("cupboard binding")
            .value;
        let Value::Object(obj) = value else {
            panic!("cupboard should be an object");
        };
        let Value::Object(pos) = obj.fields.get("pos").expect("pos object") else {
            panic!("pos should be an object");
        };
        assert_eq!(pos.fields.get("x"), Some(&Value::Number(4.0)));
        assert_eq!(pos.fields.get("y"), Some(&Value::Number(-0.5)));
        assert_eq!(pos.fields.get("z"), Some(&Value::Number(-3.25)));
    }

    #[test]
    fn supports_layout_custom_anchor_names() {
        let source = r#"
            let character = Box {
              size: vec3(2.0, 4.0, 1.0),
              anchors: {
                FootLeft: vec3(-0.4, -2.0, 0.0)
              }
            };
            var shoe = Box {
              size: vec3(0.8, 0.4, 1.0),
              anchors: {
                Mount: vec3(0.0, -0.2, 0.0)
              }
            }
              .attach(character, "FootLeft", "Mount");
        "#;
        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");
        let value = &state.bindings.get("shoe").expect("shoe binding").value;
        let Value::Object(obj) = value else {
            panic!("shoe should be an object");
        };
        let Value::Object(pos) = obj.fields.get("pos").expect("pos object") else {
            panic!("pos should be an object");
        };
        assert_eq!(pos.fields.get("x"), Some(&Value::Number(-0.4)));
        assert_eq!(pos.fields.get("y"), Some(&Value::Number(-1.8)));
        assert_eq!(pos.fields.get("z"), Some(&Value::Number(0.0)));
    }

    #[test]
    fn supports_asset_defined_anchor_defaults() {
        let source = r#"
            sdf Cabinet {
              let width = 2.0;
              let height = 3.0;
              let depth = 1.0;
              let anchors = {
                TopSurface: vec3(0.0, height * 0.5, 0.0)
              };

              fn bounds() {
                return vec3(width * 0.5, height * 0.5, depth * 0.5);
              }

              fn distance(p) {
                return length(p) - 1.0;
              }
            };

            var cabinet = Cabinet {};
            var vase = Sphere { radius: 0.2 }
              .attach(cabinet, "TopSurface", Bottom);
        "#;
        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");
        let value = &state.bindings.get("vase").expect("vase binding").value;
        let Value::Object(obj) = value else {
            panic!("vase should be an object");
        };
        let Value::Object(pos) = obj.fields.get("pos").expect("pos object") else {
            panic!("pos should be an object");
        };
        assert_eq!(pos.fields.get("y"), Some(&Value::Number(1.7)));
    }

    #[test]
    fn evaluates_custom_sdf_distance_with_helper_functions() {
        let source = r#"
            sdf SoftBlob {
              let warp_scale = 0.25;

              fn bounds() {
                return vec3(1.2, 1.2, 1.2);
              }

              fn warp(p) {
                return vec3(p.x, p.y + sin(p.x * 3.0) * warp_scale, p.z);
              }

              fn distance(p) {
                let q = warp(p);
                return length(q) - 1.0;
              }
            };
        "#;
        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");
        let value = eval_sdf_function(
            &state,
            "SoftBlob",
            "distance",
            Value::Object(ObjectValue {
                type_name: Some("vec3".to_string()),
                fields: HashMap::from([
                    ("x".to_string(), Value::Number(0.0)),
                    ("y".to_string(), Value::Number(1.0)),
                    ("z".to_string(), Value::Number(0.0)),
                ]),
            }),
        )
        .expect("distance evaluation should succeed");
        assert_eq!(value, Value::Number(0.0));

        let bounds = eval_sdf_zero_arg_function(&state, "SoftBlob", "bounds")
            .expect("bounds evaluation should succeed");
        let Value::Object(bounds_obj) = bounds else {
            panic!("bounds should be a vec3-like object");
        };
        assert_eq!(bounds_obj.fields.get("x"), Some(&Value::Number(1.2)));
    }

    #[test]
    fn jits_vec3_box_style_sdf_distance() {
        let source = r#"
            sdf ShellBox {
              let width = 1.6;
              let height = 2.0;
              let depth = 0.6;
              let wall_thickness = 0.05;

              fn sd_box(p, half_size) {
                let q = abs(p) - half_size;
                let outside = length(max(q, vec3(0.0)));
                let inside = min(max(q.x, max(q.y, q.z)), 0.0);
                return outside + inside;
              }

              fn distance(p) {
                let half_outer = vec3(width * 0.5, height * 0.5, depth * 0.5);
                let outer = sd_box(p, half_outer);
                let inner_half = max(
                  half_outer - vec3(wall_thickness, wall_thickness, wall_thickness),
                  vec3(0.001)
                );
                let cavity = sd_box(vec3(p.x, p.y, p.z - wall_thickness), inner_half);
                return max(outer, -cavity);
              }
            };
        "#;
        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");
        assert!(state.jitted_sdf_distance_functions.contains_key("ShellBox"));
    }

    #[test]
    fn supports_programmable_sdf_domain_and_distance_post_hooks() {
        let source = r#"
            sdf TwistedShell {
              let radius = 0.5;
              let thickness = 0.05;

              fn domain(p) {
                return rotate_y(p, p.y * 90.0);
              }

              fn distance(p) {
                return length(p) - radius;
              }

              fn distance_post(d, p) {
                return abs(d) - thickness;
              }
            };
        "#;
        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");
        assert!(
            state
                .jitted_sdf_vec3_functions
                .get("TwistedShell")
                .and_then(|functions| functions.get("domain"))
                .is_some()
        );
        let domain = eval_sdf_vec3_function_with_overrides(
            &state,
            "TwistedShell",
            "domain",
            Value::Object(ObjectValue {
                type_name: Some("vec3".to_string()),
                fields: HashMap::from([
                    ("x".to_string(), Value::Number(1.0)),
                    ("y".to_string(), Value::Number(1.0)),
                    ("z".to_string(), Value::Number(0.0)),
                ]),
            }),
            None,
        )
        .expect("domain evaluation should succeed");
        let Value::Object(obj) = domain else {
            panic!("domain should return vec3");
        };
        assert!(obj.fields.contains_key("x"));

        let post = eval_sdf_function_args_with_overrides(
            &state,
            "TwistedShell",
            "distance_post",
            vec![
                Value::Number(0.2),
                Value::Object(ObjectValue {
                    type_name: Some("vec3".to_string()),
                    fields: HashMap::from([
                        ("x".to_string(), Value::Number(0.0)),
                        ("y".to_string(), Value::Number(0.0)),
                        ("z".to_string(), Value::Number(0.0)),
                    ]),
                }),
            ],
            None,
        )
        .expect("distance_post should evaluate");
        assert_eq!(post, Value::Number(0.15000000000000002));
    }

    #[test]
    fn supports_object_level_modifier_function_assignments() {
        let source = r#"
            var statue = Box { size: vec3(1.0, 2.0, 1.0) };
            statue.domain = fn(p) {
              return rotate_y(p, p.y * 10.0);
            };
            statue.distance_post = fn(d, p) {
              return abs(d + sin(p.y * 20.0) * 0.01) - 0.02;
            };
        "#;
        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");
        let Value::Object(statue) = &state.bindings.get("statue").expect("statue").value else {
            panic!("statue should be an object");
        };
        assert!(matches!(
            statue.fields.get("domain"),
            Some(Value::Function(_))
        ));
        assert!(matches!(
            statue.fields.get("distance_post"),
            Some(Value::Function(_))
        ));
    }

    #[test]
    fn evaluates_top_level_functions() {
        let source = r#"
            fn twice(x) {
              return x * 2.0;
            }

            fn add(a, b) {
              return a + b;
            }

            fn accent() {
              return #ebc757;
            }

            let a = twice(3.0);
            let c = add(1.5, 2.5);
            let b = accent();
        "#;
        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");
        assert!(state.compiled_functions.contains_key("twice"));
        assert!(state.compiled_functions.contains_key("add"));
        assert!(state.compiled_functions.contains_key("accent"));
        assert!(state.jitted_functions.contains_key("twice"));
        assert!(state.jitted_functions.contains_key("add"));
        assert!(!state.jitted_functions.contains_key("accent"));
        assert_eq!(
            eval_top_level_function(&state, "twice", &[Value::Number(4.0)])
                .expect("function should evaluate"),
            Value::Number(8.0)
        );
        assert_eq!(
            eval_top_level_function(&state, "add", &[Value::Number(2.0), Value::Number(5.0)])
                .expect("function should evaluate"),
            Value::Number(7.0)
        );
        let Value::Object(color) = state.bindings.get("b").expect("b binding").value.clone() else {
            panic!("b should be a color");
        };
        assert_eq!(color.fields.get("x"), Some(&Value::Number(235.0 / 255.0)));
        assert_eq!(
            state.bindings.get("c").expect("c binding").value,
            Value::Number(4.0)
        );
    }

    #[test]
    fn parses_import_statement() {
        let program = parse_program("import \"materials/gold.ft\";").expect("parse should work");
        assert!(matches!(
            program.statements.first(),
            Some(super::Statement::Import { path, alias: None }) if path == "materials/gold.ft"
        ));
    }

    #[test]
    fn parses_import_alias_statement() {
        let program =
            parse_program("import \"materials/gold.ft\" as gold;").expect("parse should work");
        assert!(matches!(
            program.statements.first(),
            Some(super::Statement::Import { path, alias: Some(alias) })
                if path == "materials/gold.ft" && alias == "gold"
        ));
    }

    #[test]
    fn parses_export_statement() {
        let program = parse_program("export { Gold, camera };").expect("parse should work");
        assert!(matches!(
            program.statements.first(),
            Some(super::Statement::Export(names)) if names == &vec!["Gold".to_string(), "camera".to_string()]
        ));
    }

    #[test]
    fn parses_environment_statement() {
        let program = parse_program(
            r#"
            environment Sky {
              fn color(dir) {
                return vec3(0.1, 0.2, 0.3);
              }
            };
            "#,
        )
        .expect("parse should work");
        assert!(matches!(
            program.statements.first(),
            Some(super::Statement::EnvironmentDef(def)) if def.name == "Sky"
        ));
    }

    #[test]
    fn evaluates_environment_function() {
        let source = r#"
            environment Sky {
              let zenith = #4d74c7;
              let horizon = #d8e7ff;

              fn color(dir) {
                let t = clamp(dir.y * 0.5 + 0.5, 0.0, 1.0);
                return mix(horizon, zenith, t);
              }
            };
        "#;
        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");
        let value = eval_environment_function(
            &state,
            "Sky",
            "color",
            &[Value::Object(ObjectValue {
                type_name: Some("vec3".to_string()),
                fields: HashMap::from([
                    ("x".to_string(), Value::Number(0.0)),
                    ("y".to_string(), Value::Number(1.0)),
                    ("z".to_string(), Value::Number(0.0)),
                ]),
            })],
        )
        .expect("environment function should evaluate");
        let Value::Object(color) = value else {
            panic!("environment color should be vec3");
        };
        assert!(matches!(color.fields.get("z"), Some(Value::Number(v)) if *v > 0.7));
    }

    #[test]
    fn loads_relative_imports_once() {
        let dir = temp_test_dir("imports_once");
        fs::create_dir_all(&dir).expect("temp dir should exist");
        fs::write(
            dir.join("shared.ft"),
            "let gold = Metal { color: vec3(0.9, 0.7, 0.3), roughness: 0.2 };",
        )
        .expect("shared import should write");
        fs::write(
            dir.join("a.ft"),
            "import \"./shared.ft\"; let left = Sphere { material: gold };",
        )
        .expect("a.ft should write");
        fs::write(
            dir.join("b.ft"),
            "import \"./shared.ft\"; let right = Sphere { material: gold };",
        )
        .expect("b.ft should write");
        fs::write(
            dir.join("main.ft"),
            "import \"./a.ft\"; import \"./b.ft\"; let scene = left + right;",
        )
        .expect("main.ft should write");

        let program = load_program_with_imports(&dir.join("main.ft")).expect("imports should load");
        let gold_defs = program
            .statements
            .iter()
            .filter(|stmt| matches!(stmt, super::Statement::Binding { name, .. } if name == "gold"))
            .count();
        assert_eq!(gold_defs, 1);

        let state = load_and_eval_scene(&dir.join("main.ft")).expect("scene should eval");
        assert!(state.bindings.contains_key("scene"));
    }

    #[test]
    fn loads_builtin_library_imports() {
        let dir = temp_test_dir("imports_builtin");
        fs::create_dir_all(&dir).expect("temp dir should exist");
        fs::write(
            dir.join("main.ft"),
            "import \"materials/gold.ft\"; let scene = Sphere { material: Gold {} };",
        )
        .expect("main.ft should write");

        let state = load_and_eval_scene(&dir.join("main.ft")).expect("builtin import should eval");
        assert!(state.material_defs.contains_key("Gold"));
    }

    #[test]
    fn loads_builtin_library_imports_by_name() {
        let dir = temp_test_dir("imports_builtin_name");
        fs::create_dir_all(&dir).expect("temp dir should exist");
        fs::write(
            dir.join("main.ft"),
            "import \"Glass\"; let scene = Sphere { material: Glass {} };",
        )
        .expect("main.ft should write");

        let state =
            load_and_eval_scene(&dir.join("main.ft")).expect("named builtin import should eval");
        assert!(state.material_defs.contains_key("Glass"));
        assert!(state.bindings.contains_key("scene"));
    }

    #[test]
    fn loads_namespaced_imports() {
        let dir = temp_test_dir("imports_alias");
        fs::create_dir_all(&dir).expect("temp dir should exist");
        fs::write(
            dir.join("main.ft"),
            r#"
            import "materials/gold.ft" as gold;
            import "objects/soft_blob.ft" as blob;
            let scene = blob.SoftBlob {
              material: gold.Gold {}
            };
            "#,
        )
        .expect("main.ft should write");

        let program = load_program_with_imports(&dir.join("main.ft")).expect("imports should load");
        assert!(program.statements.iter().any(
            |stmt| matches!(stmt, super::Statement::MaterialDef(def) if def.name == "gold.Gold")
        ));
        assert!(program.statements.iter().any(
            |stmt| matches!(stmt, super::Statement::SdfDef(def) if def.name == "blob.SoftBlob")
        ));

        let state = load_and_eval_scene(&dir.join("main.ft")).expect("scene should eval");
        assert!(state.material_defs.contains_key("gold.Gold"));
        assert!(state.sdf_defs.contains_key("blob.SoftBlob"));
        assert!(state.bindings.contains_key("scene"));
    }

    #[test]
    fn loads_same_import_under_multiple_aliases() {
        let dir = temp_test_dir("imports_multi_alias");
        fs::create_dir_all(&dir).expect("temp dir should exist");
        fs::write(
            dir.join("shared.ft"),
            "let mat = Metal { color: vec3(0.9, 0.7, 0.3), roughness: 0.2 };",
        )
        .expect("shared.ft should write");
        fs::write(
            dir.join("main.ft"),
            r#"
            import "./shared.ft" as warm;
            import "./shared.ft" as copy;
            let left = Sphere { material: warm.mat };
            let right = Sphere { material: copy.mat };
            let scene = left + right;
            "#,
        )
        .expect("main.ft should write");

        let state = load_and_eval_scene(&dir.join("main.ft")).expect("scene should eval");
        assert!(state.bindings.contains_key("warm.mat"));
        assert!(state.bindings.contains_key("copy.mat"));
        assert!(state.bindings.contains_key("scene"));
    }

    #[test]
    fn loads_namespaced_imported_functions() {
        let dir = temp_test_dir("imports_functions");
        fs::create_dir_all(&dir).expect("temp dir should exist");
        fs::write(
            dir.join("helpers.ft"),
            r#"
            fn accent() {
              return #ebc757;
            }

            fn make_gold() {
              return Metal { color: accent(), roughness: 0.2 };
            }

            export { make_gold };
            "#,
        )
        .expect("helpers.ft should write");
        fs::write(
            dir.join("main.ft"),
            r#"
            import "./helpers.ft" as palette;
            let scene = Sphere { material: palette.make_gold() };
            "#,
        )
        .expect("main.ft should write");

        let state = load_and_eval_scene(&dir.join("main.ft")).expect("scene should eval");
        assert!(state.function_defs.contains_key("palette.make_gold"));
        assert!(state.function_defs.contains_key("palette.accent"));
        assert!(state.bindings.contains_key("scene"));
    }

    #[test]
    fn imports_only_exported_names() {
        let dir = temp_test_dir("exports_filter");
        fs::create_dir_all(&dir).expect("temp dir should exist");
        fs::write(
            dir.join("module.ft"),
            r#"
            let private_color = #ebc757;
            let public_mat = Metal { color: private_color, roughness: 0.2 };
            export { public_mat };
            "#,
        )
        .expect("module.ft should write");
        fs::write(
            dir.join("main.ft"),
            r#"
            import "./module.ft";
            let scene = Sphere { material: public_mat };
            "#,
        )
        .expect("main.ft should write");

        let state = load_and_eval_scene(&dir.join("main.ft")).expect("scene should eval");
        assert!(state.bindings.contains_key("public_mat"));
        assert!(state.bindings.contains_key("private_color"));
        assert!(state.bindings.contains_key("scene"));
    }

    #[test]
    fn namespaced_import_keeps_private_dependencies_working() {
        let dir = temp_test_dir("exports_namespace");
        fs::create_dir_all(&dir).expect("temp dir should exist");
        fs::write(
            dir.join("module.ft"),
            r#"
            let private_color = #ebc757;
            let public_mat = Metal { color: private_color, roughness: 0.2 };
            export { public_mat };
            "#,
        )
        .expect("module.ft should write");
        fs::write(
            dir.join("main.ft"),
            r#"
            import "./module.ft" as lib;
            let scene = Sphere { material: lib.public_mat };
            "#,
        )
        .expect("main.ft should write");

        let state = load_and_eval_scene(&dir.join("main.ft")).expect("scene should eval");
        assert!(state.bindings.contains_key("lib.public_mat"));
        assert!(state.bindings.contains_key("lib.private_color"));
        assert!(state.bindings.contains_key("scene"));
    }

    #[test]
    fn rejects_import_cycles() {
        let dir = temp_test_dir("imports_cycle");
        fs::create_dir_all(&dir).expect("temp dir should exist");
        fs::write(dir.join("a.ft"), "import \"./b.ft\"; let a = 1.0;").expect("a.ft should write");
        fs::write(dir.join("b.ft"), "import \"./a.ft\"; let b = 2.0;").expect("b.ft should write");

        let err = load_program_with_imports(&dir.join("a.ft")).expect_err("cycle should fail");
        assert!(matches!(err, CoreError::ImportCycle(_)));
    }

    #[test]
    fn material_properties_accept_instance_overrides() {
        let program = parse_program(
            r#"
            material Checker {
              model: Lambert;
              let color_a = #ffffff;
              let scale = 3.4;
              color = color_a;
              roughness = scale / 10.0;
            };
            "#,
        )
        .expect("program should parse");
        let state = eval_program(&program).expect("program should eval");
        let overrides = ObjectValue {
            type_name: Some("Checker".to_string()),
            fields: HashMap::from([
                (
                    "color_a".to_string(),
                    Value::Object(ObjectValue {
                        type_name: Some("vec3".to_string()),
                        fields: HashMap::from([
                            ("x".to_string(), Value::Number(0.2)),
                            ("y".to_string(), Value::Number(0.4)),
                            ("z".to_string(), Value::Number(0.6)),
                        ]),
                    }),
                ),
                ("scale".to_string(), Value::Number(6.0)),
            ]),
        };

        let properties =
            eval_material_properties_with_overrides(&state, "Checker", Some(&overrides))
                .expect("material properties should evaluate");
        let Value::Object(color) = properties
            .get("color")
            .expect("color property should exist")
        else {
            panic!("color should be vec3");
        };
        assert!(
            matches!(color.fields.get("x"), Some(Value::Number(v)) if (*v - 0.2).abs() < 1.0e-6)
        );
        assert!(
            matches!(properties.get("roughness"), Some(Value::Number(v)) if (v - 0.6).abs() < 1.0e-6)
        );
    }

    #[test]
    fn material_functions_accept_instance_overrides() {
        let program = parse_program(
            r#"
            material Checker {
              model: Lambert;
              let color_a = #ffffff;
              let color_b = #000000;
              let scale = 3.4;

              fn checker(p, scale) {
                let sx = step(0.0, sin(p.x * scale));
                let sz = step(0.0, sin(p.z * scale));
                return abs(sx - sz);
              }

              fn color(ctx) {
                let m = checker(ctx.local_position, scale);
                return mix(color_a, color_b, m);
              }
            };
            "#,
        )
        .expect("program should parse");
        let state = eval_program(&program).expect("program should eval");
        assert!(
            state
                .jitted_material_vec3_functions
                .get("Checker")
                .and_then(|functions| functions.get("color"))
                .is_some()
        );
        let overrides = ObjectValue {
            type_name: Some("Checker".to_string()),
            fields: HashMap::from([
                (
                    "color_a".to_string(),
                    Value::Object(ObjectValue {
                        type_name: Some("vec3".to_string()),
                        fields: HashMap::from([
                            ("x".to_string(), Value::Number(1.0)),
                            ("y".to_string(), Value::Number(0.0)),
                            ("z".to_string(), Value::Number(0.0)),
                        ]),
                    }),
                ),
                (
                    "color_b".to_string(),
                    Value::Object(ObjectValue {
                        type_name: Some("vec3".to_string()),
                        fields: HashMap::from([
                            ("x".to_string(), Value::Number(0.0)),
                            ("y".to_string(), Value::Number(0.0)),
                            ("z".to_string(), Value::Number(1.0)),
                        ]),
                    }),
                ),
                ("scale".to_string(), Value::Number(std::f64::consts::PI)),
            ]),
        };
        let ctx = Value::Object(ObjectValue {
            type_name: Some("ShadingContext".to_string()),
            fields: HashMap::from([(
                "local_position".to_string(),
                Value::Object(ObjectValue {
                    type_name: Some("vec3".to_string()),
                    fields: HashMap::from([
                        ("x".to_string(), Value::Number(1.0)),
                        ("y".to_string(), Value::Number(0.0)),
                        ("z".to_string(), Value::Number(0.0)),
                    ]),
                }),
            )]),
        });

        let value = eval_material_function_with_overrides(
            &state,
            "Checker",
            "color",
            ctx,
            Some(&overrides),
        )
        .expect("material function should evaluate");
        let Value::Object(color) = value else {
            panic!("color should be vec3");
        };
        assert!(
            matches!(color.fields.get("x"), Some(Value::Number(v)) if (*v - 1.0).abs() < 1.0e-6)
        );
        assert!(matches!(color.fields.get("z"), Some(Value::Number(v)) if v.abs() < 1.0e-6));
    }

    #[test]
    fn sdf_functions_accept_instance_overrides() {
        let program = parse_program(
            r#"
            sdf SoftBlob {
              let radius = 1.0;
              let warp_frequency = 4.0;
              let warp_amount = 0.16;

              fn bounds() {
                return vec3(radius + 0.2, radius + warp_amount + 0.2, radius + 0.1);
              }

              fn warp(p) {
                return vec3(p.x, p.y + sin(p.x * warp_frequency) * warp_amount, p.z);
              }

              fn distance(p) {
                let q = warp(p);
                return length(q) - radius;
              }
            };
            "#,
        )
        .expect("program should parse");
        let state = eval_program(&program).expect("program should eval");
        let overrides = ObjectValue {
            type_name: Some("SoftBlob".to_string()),
            fields: HashMap::from([
                ("radius".to_string(), Value::Number(2.0)),
                (
                    "warp_frequency".to_string(),
                    Value::Number(std::f64::consts::PI),
                ),
                ("warp_amount".to_string(), Value::Number(0.0)),
            ]),
        };

        let distance = eval_sdf_function_with_overrides(
            &state,
            "SoftBlob",
            "distance",
            Value::Object(ObjectValue {
                type_name: Some("vec3".to_string()),
                fields: HashMap::from([
                    ("x".to_string(), Value::Number(2.0)),
                    ("y".to_string(), Value::Number(0.0)),
                    ("z".to_string(), Value::Number(0.0)),
                ]),
            }),
            Some(&overrides),
        )
        .expect("distance should evaluate");
        assert!(matches!(distance, Value::Number(v) if v.abs() < 1.0e-6));

        let bounds = eval_sdf_zero_arg_function_with_overrides(
            &state,
            "SoftBlob",
            "bounds",
            Some(&overrides),
        )
        .expect("bounds should evaluate");
        let Value::Object(bounds) = bounds else {
            panic!("bounds should be vec3");
        };
        assert!(
            matches!(bounds.fields.get("x"), Some(Value::Number(v)) if (*v - 2.2).abs() < 1.0e-6)
        );
        assert!(
            matches!(bounds.fields.get("y"), Some(Value::Number(v)) if (*v - 2.2).abs() < 1.0e-6)
        );
    }

    #[test]
    fn jits_scalar_material_hook_with_ctx_members() {
        let source = r#"
            material Stripe {
              model: Metal;
              fn roughness(ctx) {
                let phase = sin(ctx.local_position.y * 12.0);
                let mask = smoothstep(-0.2, 0.2, phase);
                return mix(0.08, 0.32, mask);
              }
            };
        "#;
        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should eval");
        assert!(
            state
                .jitted_material_functions
                .get("Stripe")
                .and_then(|functions| functions.get("roughness"))
                .is_some()
        );

        let ctx = Value::Object(ObjectValue {
            type_name: Some("ShadingContext".to_string()),
            fields: HashMap::from([(
                "local_position".to_string(),
                Value::Object(ObjectValue {
                    type_name: Some("vec3".to_string()),
                    fields: HashMap::from([
                        ("x".to_string(), Value::Number(0.0)),
                        ("y".to_string(), Value::Number(0.25)),
                        ("z".to_string(), Value::Number(0.0)),
                    ]),
                }),
            )]),
        });

        let Value::Number(value) =
            eval_material_function_with_overrides(&state, "Stripe", "roughness", ctx, None)
                .expect("roughness should evaluate")
        else {
            panic!("roughness should be numeric");
        };
        assert!(value.is_finite());
    }

    #[test]
    fn supports_skeleton_assets_and_semantic_bones() {
        let source = r#"
            skeleton Robot {
              joint pelvis = vec3(0.0, 1.0, 0.0);
              joint neck = vec3(0.0, 1.6, 0.0);
              joint elbow_l = vec3(-0.6, 1.3, 0.0);
              joint hand_l = vec3(-0.85, 1.05, 0.0);
              bone torso = pelvis, neck;
              bone forearm_l = elbow_l, hand_l;
            };

            let robot = Robot {};
            let hand = robot.hand_l;
            let forearm = robot.forearm_l;
        "#;
        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");

        let Value::Object(robot) = &state.bindings.get("robot").expect("robot binding").value
        else {
            panic!("robot should be an object");
        };
        assert!(robot.fields.contains_key("anchors"));
        assert!(robot.fields.contains_key("__skeleton_joints"));
        assert!(robot.fields.contains_key("__skeleton_bones"));

        let Value::Object(hand) = &state.bindings.get("hand").expect("hand binding").value else {
            panic!("hand should be an object");
        };
        assert_eq!(hand.type_name.as_deref(), Some("SkeletonJoint"));
        assert!(hand.fields.contains_key("__bounds"));

        let Value::Object(forearm) = &state
            .bindings
            .get("forearm")
            .expect("forearm binding")
            .value
        else {
            panic!("forearm should be an object");
        };
        assert_eq!(forearm.type_name.as_deref(), Some("SkeletonBone"));
        let Value::Object(anchors) = forearm.fields.get("anchors").expect("anchors field") else {
            panic!("forearm anchors should be an object");
        };
        assert!(anchors.fields.contains_key("Start"));
        assert!(anchors.fields.contains_key("End"));
    }

    #[test]
    fn bind_aligns_box_long_axis_to_bone_segment() {
        fn read_vec3(obj: &ObjectValue, field: &str) -> [f64; 3] {
            let Value::Object(v) = obj.fields.get(field).expect("field should exist") else {
                panic!("field should be vec3 object");
            };
            let read = |name: &str| match v.fields.get(name).expect("component should exist") {
                Value::Number(n) => *n,
                _ => panic!("component should be numeric"),
            };
            [read("x"), read("y"), read("z")]
        }

        fn rotate_xyz(p: [f64; 3], rot_deg: [f64; 3]) -> [f64; 3] {
            let (sx, cx) = rot_deg[0].to_radians().sin_cos();
            let (sy, cy) = rot_deg[1].to_radians().sin_cos();
            let (sz, cz) = rot_deg[2].to_radians().sin_cos();

            let py = p[1] * cx - p[2] * sx;
            let pz = p[1] * sx + p[2] * cx;
            let px = p[0];

            let px2 = px * cy + pz * sy;
            let pz2 = -px * sy + pz * cy;
            let py2 = py;

            let px3 = px2 * cz - py2 * sz;
            let py3 = px2 * sz + py2 * cz;
            [px3, py3, pz2]
        }

        let source = r#"
            skeleton Rig {
              joint a = vec3(0.0, 1.6, 0.0);
              joint b = vec3(-0.3, 1.0, 0.0);
              bone arm = a, b;
            };

            let rig = Rig {};
            let part = Box {
              size: vec3(0.1, 0.1, 0.5)
            }.bind(rig.arm);
        "#;

        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");

        let Value::Object(rig) = &state.bindings.get("rig").expect("rig binding").value else {
            panic!("rig should be an object");
        };
        let Value::Object(joints) = rig
            .fields
            .get("__skeleton_joints")
            .expect("skeleton joints should exist")
        else {
            panic!("skeleton joints should be an object");
        };
        let a = match joints.fields.get("a").expect("joint a should exist") {
            Value::Object(v) => {
                let x = match v.fields.get("x").unwrap() {
                    Value::Number(n) => *n,
                    _ => 0.0,
                };
                let y = match v.fields.get("y").unwrap() {
                    Value::Number(n) => *n,
                    _ => 0.0,
                };
                let z = match v.fields.get("z").unwrap() {
                    Value::Number(n) => *n,
                    _ => 0.0,
                };
                [x, y, z]
            }
            _ => panic!("joint a should be vec3"),
        };
        let b = match joints.fields.get("b").expect("joint b should exist") {
            Value::Object(v) => {
                let x = match v.fields.get("x").unwrap() {
                    Value::Number(n) => *n,
                    _ => 0.0,
                };
                let y = match v.fields.get("y").unwrap() {
                    Value::Number(n) => *n,
                    _ => 0.0,
                };
                let z = match v.fields.get("z").unwrap() {
                    Value::Number(n) => *n,
                    _ => 0.0,
                };
                [x, y, z]
            }
            _ => panic!("joint b should be vec3"),
        };
        let rig_pos = read_vec3(rig, "pos");
        let start = [a[0] + rig_pos[0], a[1] + rig_pos[1], a[2] + rig_pos[2]];
        let end = [b[0] + rig_pos[0], b[1] + rig_pos[1], b[2] + rig_pos[2]];

        let Value::Object(part) = &state.bindings.get("part").expect("part binding").value else {
            panic!("part should be an object");
        };
        let pos = read_vec3(part, "pos");
        let rot = read_vec3(part, "rot");
        let size = read_vec3(part, "size");
        let half = size[2] * 0.5;
        let p0 = rotate_xyz([0.0, 0.0, -half], rot);
        let p1 = rotate_xyz([0.0, 0.0, half], rot);
        let end0 = [pos[0] + p0[0], pos[1] + p0[1], pos[2] + p0[2]];
        let end1 = [pos[0] + p1[0], pos[1] + p1[1], pos[2] + p1[2]];

        let dist = |u: [f64; 3], v: [f64; 3]| -> f64 {
            ((u[0] - v[0]).powi(2) + (u[1] - v[1]).powi(2) + (u[2] - v[2]).powi(2)).sqrt()
        };
        let pairing_a = dist(end0, start) + dist(end1, end);
        let pairing_b = dist(end0, end) + dist(end1, start);
        let best = pairing_a.min(pairing_b);
        assert!(
            best < 0.08,
            "bound box endpoints should match bone endpoints closely, got {best}"
        );
    }

    #[test]
    fn robot_scene_upper_arm_binding_matches_robot_bone() {
        fn read_vec3(obj: &ObjectValue, field: &str) -> [f64; 3] {
            let Value::Object(v) = obj.fields.get(field).expect("field should exist") else {
                panic!("field should be vec3 object");
            };
            let read = |name: &str| match v.fields.get(name).expect("component should exist") {
                Value::Number(n) => *n,
                _ => panic!("component should be numeric"),
            };
            [read("x"), read("y"), read("z")]
        }

        fn rotate_xyz(p: [f64; 3], rot_deg: [f64; 3]) -> [f64; 3] {
            let (sx, cx) = rot_deg[0].to_radians().sin_cos();
            let (sy, cy) = rot_deg[1].to_radians().sin_cos();
            let (sz, cz) = rot_deg[2].to_radians().sin_cos();

            let py = p[1] * cx - p[2] * sx;
            let pz = p[1] * sx + p[2] * cx;
            let px = p[0];

            let px2 = px * cy + pz * sy;
            let pz2 = -px * sy + pz * cy;
            let py2 = py;

            let px3 = px2 * cz - py2 * sz;
            let py3 = px2 * sz + py2 * cz;
            [px3, py3, pz2]
        }

        let state = load_and_eval_scene(&std::path::PathBuf::from(
            "../../examples/robot_skeleton.ft",
        ))
        .expect("robot skeleton scene should load");

        let Value::Object(rig) = &state.bindings.get("rig").expect("rig binding").value else {
            panic!("rig should be an object");
        };
        let rig_pos = read_vec3(rig, "pos");
        let Value::Object(joints) = rig
            .fields
            .get("__skeleton_joints")
            .expect("skeleton joints should exist")
        else {
            panic!("skeleton joints should be an object");
        };
        let shoulder = match joints.fields.get("shoulder_l").expect("joint should exist") {
            Value::Object(v) => {
                let x = match v.fields.get("x").unwrap() {
                    Value::Number(n) => *n,
                    _ => 0.0,
                };
                let y = match v.fields.get("y").unwrap() {
                    Value::Number(n) => *n,
                    _ => 0.0,
                };
                let z = match v.fields.get("z").unwrap() {
                    Value::Number(n) => *n,
                    _ => 0.0,
                };
                [x + rig_pos[0], y + rig_pos[1], z + rig_pos[2]]
            }
            _ => panic!("joint should be vec3"),
        };
        let elbow = match joints.fields.get("elbow_l").expect("joint should exist") {
            Value::Object(v) => {
                let x = match v.fields.get("x").unwrap() {
                    Value::Number(n) => *n,
                    _ => 0.0,
                };
                let y = match v.fields.get("y").unwrap() {
                    Value::Number(n) => *n,
                    _ => 0.0,
                };
                let z = match v.fields.get("z").unwrap() {
                    Value::Number(n) => *n,
                    _ => 0.0,
                };
                [x + rig_pos[0], y + rig_pos[1], z + rig_pos[2]]
            }
            _ => panic!("joint should be vec3"),
        };

        let Value::Object(part) = &state
            .bindings
            .get("upper_arm")
            .expect("upper_arm binding")
            .value
        else {
            panic!("upper_arm should be an object");
        };
        let pos = read_vec3(part, "pos");
        let rot = read_vec3(part, "rot");
        let size = read_vec3(part, "size");
        let half = size[2] * 0.5;
        let p0 = rotate_xyz([0.0, 0.0, -half], rot);
        let p1 = rotate_xyz([0.0, 0.0, half], rot);
        let end0 = [pos[0] + p0[0], pos[1] + p0[1], pos[2] + p0[2]];
        let end1 = [pos[0] + p1[0], pos[1] + p1[1], pos[2] + p1[2]];

        let dist = |u: [f64; 3], v: [f64; 3]| -> f64 {
            ((u[0] - v[0]).powi(2) + (u[1] - v[1]).powi(2) + (u[2] - v[2]).powi(2)).sqrt()
        };
        let pairing_a = dist(end0, shoulder) + dist(end1, elbow);
        let pairing_b = dist(end0, elbow) + dist(end1, shoulder);
        let best = pairing_a.min(pairing_b);
        assert!(
            best < 0.10,
            "robot upper arm endpoints should match shoulder/elbow, got {best}"
        );
    }

    #[test]
    fn skeleton_ik_preserves_two_bone_chain_lengths() {
        let source = r#"
            skeleton Rig {
              joint shoulder = vec3(0.0, 1.6, 0.0);
              joint elbow = vec3(-0.2, 1.0, 0.0);
              joint hand = vec3(-0.4, 0.5, 0.0);
              chain arm = shoulder, elbow, hand;
            };

            let rig = Rig {
              ik: {
                arm: vec3(-0.6, 0.1, 0.2)
              }
            };
        "#;

        let program = parse_program(source).expect("program should parse");
        let state = eval_program(&program).expect("program should evaluate");
        let Value::Object(rig) = &state.bindings.get("rig").expect("rig binding").value else {
            panic!("rig should be an object");
        };
        let Value::Object(joints) = rig
            .fields
            .get("__skeleton_joints")
            .expect("joints should exist")
        else {
            panic!("joints should be an object");
        };

        let joint = |name: &str| -> [f64; 3] {
            let Value::Object(v) = joints.fields.get(name).expect("joint should exist") else {
                panic!("joint should be vec3");
            };
            let read = |field: &str| match v.fields.get(field).expect("component should exist") {
                Value::Number(n) => *n,
                _ => panic!("component should be numeric"),
            };
            [read("x"), read("y"), read("z")]
        };

        let shoulder = joint("shoulder");
        let elbow = joint("elbow");
        let hand = joint("hand");

        let dist = |a: [f64; 3], b: [f64; 3]| -> f64 {
            ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
        };

        let upper = dist(shoulder, elbow);
        let lower = dist(elbow, hand);
        assert!(
            (upper - 0.632455532).abs() < 0.02,
            "upper chain length should stay fixed, got {upper}"
        );
        assert!(
            (lower - 0.538516481).abs() < 0.02,
            "lower chain length should stay fixed, got {lower}"
        );
    }

    fn temp_test_dir(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should work")
            .as_nanos();
        std::env::temp_dir().join(format!("forgedthoughts-{label}-{stamp}"))
    }
}
