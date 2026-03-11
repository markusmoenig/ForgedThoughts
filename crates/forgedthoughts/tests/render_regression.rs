use std::{
    env, fs,
    path::{Path, PathBuf},
};

use forgedthoughts::{
    AccelMode, EvalState, RaySettings, RenderOptions, eval_program, parse_program,
    render_preview_progressive_with_accel, render_ray_progressive_with_accel,
};
use image::{ImageReader, RgbImage};

#[derive(Clone, Copy)]
enum RegressionRenderer {
    Preview,
    Ray { max_depth: u32, aa_samples: u32 },
}

#[derive(Clone, Copy)]
struct RegressionCase {
    name: &'static str,
    scene_file: &'static str,
    renderer: RegressionRenderer,
    accel: AccelMode,
    options: RenderOptions,
}

#[test]
fn render_regression_fixtures() {
    for case in regression_cases() {
        run_case(case);
    }
}

fn regression_cases() -> Vec<RegressionCase> {
    vec![
        RegressionCase {
            name: "preview_lambert",
            scene_file: "preview_lambert.ft",
            renderer: RegressionRenderer::Preview,
            accel: AccelMode::Bvh,
            options: RenderOptions {
                width: 96,
                height: 96,
                max_steps: 320,
                max_dist: 40.0,
                epsilon: 0.0002,
                step_scale: 0.7,
                camera_z: 6.0,
                fov_y_degrees: 35.0,
            },
        },
        RegressionCase {
            name: "ray_metal",
            scene_file: "ray_metal.ft",
            renderer: RegressionRenderer::Ray {
                max_depth: 4,
                aa_samples: 1,
            },
            accel: AccelMode::Bvh,
            options: RenderOptions {
                width: 96,
                height: 96,
                max_steps: 360,
                max_dist: 50.0,
                epsilon: 0.0002,
                step_scale: 0.7,
                camera_z: 6.0,
                fov_y_degrees: 35.0,
            },
        },
        RegressionCase {
            name: "ray_boolean_union_round",
            scene_file: "ray_boolean_union_round.ft",
            renderer: RegressionRenderer::Ray {
                max_depth: 4,
                aa_samples: 1,
            },
            accel: AccelMode::Bvh,
            options: RenderOptions {
                width: 96,
                height: 96,
                max_steps: 520,
                max_dist: 60.0,
                epsilon: 0.0002,
                step_scale: 0.7,
                camera_z: 6.0,
                fov_y_degrees: 25.0,
            },
        },
        RegressionCase {
            name: "preview_material_hook",
            scene_file: "preview_material_hook.ft",
            renderer: RegressionRenderer::Preview,
            accel: AccelMode::Bvh,
            options: RenderOptions {
                width: 96,
                height: 96,
                max_steps: 320,
                max_dist: 40.0,
                epsilon: 0.0002,
                step_scale: 0.7,
                camera_z: 6.0,
                fov_y_degrees: 35.0,
            },
        },
    ]
}

fn run_case(case: RegressionCase) {
    let scene_path = fixture_root().join(case.scene_file);
    let source = fs::read_to_string(&scene_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", scene_path.display()));
    let state = parse_and_eval(&source, &scene_path);
    let actual = render_case(&state, case);
    let expected_path = fixture_root().join(format!("{}.png", case.name));

    if should_update_goldens() {
        fs::create_dir_all(fixture_root()).expect("render fixture directory should be writable");
        actual
            .save(&expected_path)
            .unwrap_or_else(|err| panic!("failed to write {}: {err}", expected_path.display()));
        return;
    }

    let expected = ImageReader::open(&expected_path)
        .unwrap_or_else(|err| panic!("failed to open {}: {err}", expected_path.display()))
        .decode()
        .unwrap_or_else(|err| panic!("failed to decode {}: {err}", expected_path.display()))
        .to_rgb8();

    assert_eq!(
        actual.dimensions(),
        expected.dimensions(),
        "fixture '{}' dimensions changed",
        case.name
    );

    let metrics = diff_metrics(&actual, &expected);
    assert!(
        metrics.max_abs_diff <= 8 && metrics.mean_abs_diff <= 0.35,
        "fixture '{}' diverged: max_abs_diff={}, mean_abs_diff={:.4}",
        case.name,
        metrics.max_abs_diff,
        metrics.mean_abs_diff
    );
}

fn parse_and_eval(source: &str, scene_path: &Path) -> EvalState {
    let program = parse_program(source)
        .unwrap_or_else(|err| panic!("failed to parse {}: {err}", scene_path.display()));
    eval_program(&program)
        .unwrap_or_else(|err| panic!("failed to eval {}: {err}", scene_path.display()))
}

fn render_case(state: &EvalState, case: RegressionCase) -> RgbImage {
    match case.renderer {
        RegressionRenderer::Preview => render_preview_progressive_with_accel(
            state,
            case.options,
            case.accel,
            case.options.width.max(case.options.height),
            1,
            |_, _| Ok(()),
        )
        .unwrap_or_else(|err| panic!("failed to render preview fixture '{}': {err}", case.name)),
        RegressionRenderer::Ray {
            max_depth,
            aa_samples,
        } => render_ray_progressive_with_accel(
            state,
            case.options,
            case.accel,
            RaySettings {
                max_depth,
                tile_size: case.options.width.max(case.options.height),
                aa_samples,
                debug_aov: None,
            },
            |_, _| Ok(()),
        )
        .unwrap_or_else(|err| panic!("failed to render ray fixture '{}': {err}", case.name)),
    }
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("render_regression")
}

fn should_update_goldens() -> bool {
    env::var_os("UPDATE_RENDER_GOLDENS").is_some()
}

struct DiffMetrics {
    max_abs_diff: u8,
    mean_abs_diff: f32,
}

fn diff_metrics(actual: &RgbImage, expected: &RgbImage) -> DiffMetrics {
    let mut max_abs_diff = 0u8;
    let mut sum_abs_diff = 0u64;
    let mut samples = 0u64;

    for (actual_px, expected_px) in actual.pixels().zip(expected.pixels()) {
        for (a, e) in actual_px.0.iter().zip(expected_px.0.iter()) {
            let diff = a.abs_diff(*e);
            max_abs_diff = max_abs_diff.max(diff);
            sum_abs_diff += u64::from(diff);
            samples += 1;
        }
    }

    DiffMetrics {
        max_abs_diff,
        mean_abs_diff: sum_abs_diff as f32 / samples as f32,
    }
}
