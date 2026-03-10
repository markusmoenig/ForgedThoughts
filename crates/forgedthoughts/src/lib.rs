mod ast;
mod eval;
mod lexer;
mod materials;
mod parser;
mod render_api;
mod renderer;

use std::{
    collections::{HashMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
};

pub use ast::{BinaryOp, Expr, Program, Statement, UnaryOp};
pub use eval::{
    Binding, EvalError, EvalState, ObjectValue, Value, eval_environment_function,
    eval_material_function, eval_material_properties, eval_program, eval_sdf_function,
    eval_sdf_zero_arg_function, eval_top_level_function,
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
    AccelMode, PathtraceProgress, PathtraceSettings, PreviewProgress, RayDebugAov, RayProgress,
    RaySettings, RenderError, RenderOptions, SceneRenderSettings, extract_scene_render_settings,
    render_depth_png, render_depth_png_with_accel, render_pathtrace_png_with_accel,
    render_pathtrace_progressive_with_accel, render_preview_progressive_with_accel,
    render_ray_png_with_accel, render_ray_progressive_with_accel,
};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinLibraryCategory {
    Materials,
    Objects,
    Scenes,
}

#[derive(Debug, Clone, Copy)]
pub struct BuiltinLibraryItem {
    pub category: BuiltinLibraryCategory,
    pub name: &'static str,
    pub path: &'static str,
    pub source: &'static str,
}

const BUILTIN_LIBRARY: &[BuiltinLibraryItem] = &[
    BuiltinLibraryItem {
        category: BuiltinLibraryCategory::Materials,
        name: "Gold",
        path: "materials/gold.ft",
        source: include_str!("../library/materials/gold.ft"),
    },
    BuiltinLibraryItem {
        category: BuiltinLibraryCategory::Materials,
        name: "Glass",
        path: "materials/glass.ft",
        source: include_str!("../library/materials/glass.ft"),
    },
    BuiltinLibraryItem {
        category: BuiltinLibraryCategory::Objects,
        name: "SoftBlob",
        path: "objects/soft_blob.ft",
        source: include_str!("../library/objects/soft_blob.ft"),
    },
    BuiltinLibraryItem {
        category: BuiltinLibraryCategory::Scenes,
        name: "Studio",
        path: "scenes/studio.ft",
        source: include_str!("../library/scenes/studio.ft"),
    },
];

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
        Statement::EnvironmentDef(def) => {
            let mut deps = HashSet::new();
            let mut scope = HashSet::new();
            for stmt in &def.statements {
                if let ast::MaterialStatement::Binding { name, .. } = stmt {
                    scope.insert(name.clone());
                }
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
        CoreError, ObjectValue, Value, eval_environment_function, eval_program, eval_sdf_function,
        eval_sdf_zero_arg_function, eval_top_level_function, load_and_eval_scene,
        load_program_with_imports, parse_program,
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

    fn temp_test_dir(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should work")
            .as_nanos();
        std::env::temp_dir().join(format!("forgedthoughts-{label}-{stamp}"))
    }
}
