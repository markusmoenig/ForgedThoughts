use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
};

use forgedthoughts::{EvalState, Value, eval_program, parse_program};

#[test]
fn parse_golden_fixtures() {
    for fixture in list_ft_files("tests/fixtures/parse_ok") {
        let source = read(&fixture);
        let program = parse_program(&source).expect("parse fixture should parse");
        let actual = format!("{program:#?}\n");
        let expected = read_sidecar(&fixture, "ast");
        assert_eq!(actual, expected, "fixture: {}", fixture.display());
    }
}

#[test]
fn eval_golden_fixtures() {
    for fixture in list_ft_files("tests/fixtures/eval_ok") {
        let source = read(&fixture);
        let program = parse_program(&source).expect("eval fixture should parse");
        let state = eval_program(&program).expect("eval fixture should evaluate");
        let actual = render_eval_state(&state);
        let expected = read_sidecar(&fixture, "state");
        assert_eq!(actual, expected, "fixture: {}", fixture.display());
    }
}

#[test]
fn parse_error_fixtures() {
    for fixture in list_ft_files("tests/fixtures/parse_err") {
        let source = read(&fixture);
        let err = parse_program(&source).expect_err("fixture should fail parse");
        let actual = format!("{err}\n");
        let expected = read_sidecar(&fixture, "err");
        assert_eq!(actual, expected, "fixture: {}", fixture.display());
    }
}

#[test]
fn eval_error_fixtures() {
    for fixture in list_ft_files("tests/fixtures/eval_err") {
        let source = read(&fixture);
        let program = parse_program(&source).expect("fixture should parse");
        let err = eval_program(&program).expect_err("fixture should fail eval");
        let actual = format!("{err}\n");
        let expected = read_sidecar(&fixture, "err");
        assert_eq!(actual, expected, "fixture: {}", fixture.display());
    }
}

fn list_ft_files(relative_dir: &str) -> Vec<PathBuf> {
    let mut entries = Vec::new();
    let root = fixture_root().join(relative_dir);
    for entry in fs::read_dir(&root).expect("fixture directory should be readable") {
        let path = entry.expect("fixture entry should be readable").path();
        if path.extension().and_then(|e| e.to_str()) == Some("ft") {
            entries.push(path);
        }
    }
    entries.sort();
    entries
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn read(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
}

fn read_sidecar(path: &Path, ext: &str) -> String {
    let sidecar = path.with_extension(ext);
    read(&sidecar)
}

fn render_eval_state(state: &EvalState) -> String {
    let mut out = String::new();
    let mut bindings = BTreeMap::new();
    for (name, binding) in &state.bindings {
        bindings.insert(name.as_str(), binding);
    }

    for (name, binding) in bindings {
        out.push_str(name);
        out.push_str(" (");
        out.push_str(if binding.mutable { "var" } else { "let" });
        out.push_str(") = ");
        out.push_str(&render_value(&binding.value));
        out.push('\n');
    }

    out
}

fn render_value(value: &Value) -> String {
    match value {
        Value::Number(n) => format!("{n}"),
        Value::Object(obj) => {
            let mut out = String::new();
            out.push_str("Object(");
            out.push_str(obj.type_name.as_deref().unwrap_or("anonymous"));
            out.push_str(") {");

            let sorted = sort_fields(&obj.fields);
            let mut first = true;
            for (k, v) in sorted {
                if !first {
                    out.push_str(", ");
                }
                first = false;
                out.push_str(&k);
                out.push_str(": ");
                out.push_str(&render_value(&v));
            }

            out.push('}');
            out
        }
    }
}

fn sort_fields(fields: &HashMap<String, Value>) -> BTreeMap<String, Value> {
    let mut sorted = BTreeMap::new();
    for (k, v) in fields {
        sorted.insert(k.clone(), v.clone());
    }
    sorted
}
