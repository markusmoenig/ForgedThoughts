use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use clap::{Parser, Subcommand, ValueEnum};
use forgedthoughts::{
    AccelMode, AppConfig, BuiltinLibraryCategory, CoreError, RayDebugAov, RaySettings,
    RenderOptions, SceneRenderSettings, builtin_library_item_metadata, builtin_library_items,
    extract_scene_render_settings, load_and_eval_scene, render_depth_png_with_accel,
    render_preview_progressive_with_accel, render_ray_progressive_with_accel, resolve_scene_path,
};
use indicatif::{ProgressBar, ProgressStyle};
use tracing::{error, info, level_filters::LevelFilter};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(name = "ftc")]
#[command(
    about = "CLI for ForgedThoughts scene tools. Without a subcommand, ftc runs the trace renderer."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    #[command(flatten)]
    default_ray: DefaultRayArgs,

    /// Increase log output (-v, -vv)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Parse and evaluate a scene without rendering
    Check {
        /// Path to a .ft scene file
        #[arg(short, long)]
        scene: Option<PathBuf>,

        /// Re-run when the scene file changes
        #[arg(long)]
        watch: bool,
    },
    /// Render a classical Whitted-style traced PNG from a scene
    #[command(alias = "ray")]
    Trace {
        /// Path to a .ft scene file
        #[arg(short, long)]
        scene: Option<PathBuf>,

        /// Output PNG path (default: <scene>.png)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Render width
        #[arg(long)]
        width: Option<u32>,

        /// Render height
        #[arg(long)]
        height: Option<u32>,

        /// Acceleration backend
        #[arg(long, value_enum)]
        accel: Option<CliAccelMode>,

        /// Max recursive depth
        #[arg(long, default_value_t = 8)]
        depth: u32,

        /// Tile size for progressive updates
        #[arg(long, default_value_t = 64)]
        tile_size: u32,

        /// Camera samples per pixel for anti-aliasing
        #[arg(long, default_value_t = 1)]
        aa: u32,

        /// Debug AOV output (replaces beauty)
        #[arg(long, value_enum)]
        debug_aov: Option<CliRayDebugAov>,

        /// Re-render when the scene file changes
        #[arg(long)]
        watch: bool,
    },
    /// Render a fast depth preview PNG from a scene
    #[command(alias = "render")]
    Depth {
        /// Path to a .ft scene file
        #[arg(short, long)]
        scene: Option<PathBuf>,

        /// Output PNG path (default: <scene>.png)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Render width
        #[arg(long)]
        width: Option<u32>,

        /// Render height
        #[arg(long)]
        height: Option<u32>,

        /// Acceleration backend
        #[arg(long, value_enum)]
        accel: Option<CliAccelMode>,

        /// Tile size for progressive preview updates
        #[arg(long, default_value_t = 64)]
        tile_size: u32,

        /// Camera samples per pixel for anti-aliasing
        #[arg(long, default_value_t = 1)]
        aa: u32,

        /// Re-render when the scene file changes
        #[arg(long)]
        watch: bool,
    },
    /// Benchmark all acceleration backends on the same scene
    Bench {
        /// Path to a .ft scene file
        #[arg(short, long)]
        scene: Option<PathBuf>,

        /// Render width
        #[arg(long)]
        width: Option<u32>,

        /// Render height
        #[arg(long)]
        height: Option<u32>,

        /// Number of measured iterations per backend
        #[arg(long, default_value_t = 5)]
        iterations: u32,

        /// Number of warmup iterations per backend
        #[arg(long, default_value_t = 1)]
        warmup: u32,
    },
    /// List built-in library assets
    List {
        #[command(subcommand)]
        kind: ListCommand,
    },
}

#[derive(Debug, Subcommand)]
enum ListCommand {
    Materials,
    Objects,
    Skeletons,
    Scenes,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliAccelMode {
    Naive,
    Bvh,
    Bricks,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliRayDebugAov {
    Depth,
    Normal,
    MaterialId,
    Ior,
    Transmission,
    Fresnel,
    HitT,
}

#[derive(Debug, clap::Args)]
struct DefaultRayArgs {
    /// Path to a .ft scene file
    #[arg(short, long)]
    scene: Option<PathBuf>,

    /// Output PNG path (default: <scene>.png)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Render width
    #[arg(long)]
    width: Option<u32>,

    /// Render height
    #[arg(long)]
    height: Option<u32>,

    /// Acceleration backend
    #[arg(long, value_enum)]
    accel: Option<CliAccelMode>,

    /// Max recursive depth
    #[arg(long, default_value_t = 8)]
    depth: u32,

    /// Tile size for progressive updates
    #[arg(long, default_value_t = 64)]
    tile_size: u32,

    /// Camera samples per pixel for anti-aliasing
    #[arg(long, default_value_t = 1)]
    aa: u32,

    /// Debug AOV output (replaces beauty)
    #[arg(long, value_enum)]
    debug_aov: Option<CliRayDebugAov>,

    /// Re-render when the scene file changes
    #[arg(long)]
    watch: bool,
}

impl From<CliAccelMode> for AccelMode {
    fn from(value: CliAccelMode) -> Self {
        match value {
            CliAccelMode::Naive => Self::Naive,
            CliAccelMode::Bvh => Self::Bvh,
            CliAccelMode::Bricks => Self::Bricks,
        }
    }
}

impl From<CliRayDebugAov> for RayDebugAov {
    fn from(value: CliRayDebugAov) -> Self {
        match value {
            CliRayDebugAov::Depth => Self::Depth,
            CliRayDebugAov::Normal => Self::Normal,
            CliRayDebugAov::MaterialId => Self::MaterialId,
            CliRayDebugAov::Ior => Self::Ior,
            CliRayDebugAov::Transmission => Self::Transmission,
            CliRayDebugAov::Fresnel => Self::Fresnel,
            CliRayDebugAov::HitT => Self::HitT,
        }
    }
}

fn init_logging(verbose: u8) {
    let level = match verbose {
        0 => LevelFilter::INFO,
        1 => LevelFilter::DEBUG,
        _ => LevelFilter::TRACE,
    };

    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::WARN.into())
        .from_env_lossy()
        .add_directive(format!("ftc={level}").parse().expect("valid ftc log level"))
        .add_directive(
            format!("forgedthoughts={level}")
                .parse()
                .expect("valid forgedthoughts log level"),
        );

    tracing_subscriber::fmt().with_env_filter(filter).init();
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    init_logging(cli.verbose);

    let cfg = AppConfig::from_env();
    match cli.command {
        None => run_ray(cli.default_ray.into_params(), &cfg),
        Some(Command::Check { scene, watch }) => run_check(scene, watch, &cfg),
        Some(Command::Depth {
            scene,
            output,
            width,
            height,
            accel,
            tile_size,
            aa,
            watch,
        }) => run_render(
            RenderParams {
                scene,
                output,
                width,
                height,
                accel: accel.map(Into::into),
                tile_size,
                aa,
                watch,
            },
            &cfg,
        ),
        Some(Command::Trace {
            scene,
            output,
            width,
            height,
            accel,
            depth,
            tile_size,
            aa,
            debug_aov,
            watch,
        }) => run_ray(
            RayParams {
                scene,
                output,
                width,
                height,
                accel: accel.map(Into::into),
                depth,
                tile_size,
                aa,
                debug_aov: debug_aov.map(Into::into),
                watch,
            },
            &cfg,
        ),
        Some(Command::Bench {
            scene,
            width,
            height,
            iterations,
            warmup,
        }) => run_bench(scene, width, height, iterations, warmup, &cfg),
        Some(Command::List { kind }) => run_list(kind),
    }
}

impl DefaultRayArgs {
    fn into_params(self) -> RayParams {
        RayParams {
            scene: self.scene,
            output: self.output,
            width: self.width,
            height: self.height,
            accel: self.accel.map(Into::into),
            depth: self.depth,
            tile_size: self.tile_size,
            aa: self.aa,
            debug_aov: self.debug_aov.map(Into::into),
            watch: self.watch,
        }
    }
}

fn run_check(scene: Option<PathBuf>, watch: bool, cfg: &AppConfig) -> ExitCode {
    run_with_watch(scene, watch, cfg, run_check_once)
}

fn run_check_once(scene_path: &Path) -> ExitCode {
    match load_and_eval_scene(scene_path) {
        Ok(state) => {
            info!(
                scene = %scene_path.display(),
                bindings = state.bindings.len(),
                "scene parsed and evaluated"
            );
            info!("check completed");
            ExitCode::SUCCESS
        }
        Err(err) => {
            error!(scene = %scene_path.display(), "{err}");
            ExitCode::from(3)
        }
    }
}

fn run_render(params: RenderParams, cfg: &AppConfig) -> ExitCode {
    run_with_watch(params.scene.clone(), params.watch, cfg, |scene_path| {
        run_render_once(scene_path, &params)
    })
}

fn run_render_once(scene_path: &Path, params: &RenderParams) -> ExitCode {
    match load_and_eval_scene(scene_path) {
        Ok(state) => {
            let scene_settings = extract_scene_render_settings(&state);
            let options = merged_render_options(&scene_settings, params.width, params.height);
            let accel = params
                .accel
                .or(scene_settings.accel)
                .unwrap_or(AccelMode::Bvh);
            info!(
                scene = %scene_path.display(),
                bindings = state.bindings.len(),
                "scene parsed and evaluated"
            );

            let output_path = params
                .output
                .as_deref()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| default_output_path(scene_path));
            let tile_size = params.tile_size.max(8);
            let tiles_x = options.width.div_ceil(tile_size);
            let tiles_y = options.height.div_ceil(tile_size);
            let tiles_total = u64::from(tiles_x) * u64::from(tiles_y);
            let progress = ProgressBar::new(tiles_total.max(1));
            let style = ProgressStyle::with_template(
                "[{elapsed_precise}] {wide_bar} {pos}/{len} tiles {msg}",
            )
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("=>-");
            progress.set_style(style);
            let render_start = Instant::now();
            let image = match render_preview_progressive_with_accel(
                &state,
                options,
                accel,
                tile_size,
                params.aa.max(1),
                |step, image| {
                    progress.set_position(u64::from(step.tiles_done));
                    progress.set_message(format!("{} ms", step.elapsed_ms));
                    image.save(&output_path)?;
                    Ok(())
                },
            ) {
                Ok(image) => image,
                Err(err) => {
                    progress.abandon_with_message("failed");
                    error!(output = %output_path.display(), "{err}");
                    return ExitCode::from(4);
                }
            };
            if let Err(err) = image.save(&output_path) {
                progress.abandon_with_message("failed");
                error!(output = %output_path.display(), "{err}");
                return ExitCode::from(4);
            }
            progress.finish_with_message("done");
            let render_elapsed = render_start.elapsed();
            let render_secs = render_elapsed.as_secs_f64();
            let render_secs_display = format!("{render_secs:.3}s");
            let megapixels = (f64::from(options.width) * f64::from(options.height)) / 1_000_000.0;
            let mpix_per_sec = megapixels / render_secs.max(f64::EPSILON);
            let mpix_per_sec_display = format!("{mpix_per_sec:.3}");
            info!(
                output = %output_path.display(),
                width = options.width,
                height = options.height,
                accel = ?accel,
                tile_size,
                aa = params.aa.max(1),
                "depth preview rendered"
            );
            info!(
                render_secs = %render_secs_display,
                mpix_per_sec = %mpix_per_sec_display,
                "benchmark"
            );
            ExitCode::SUCCESS
        }
        Err(err) => {
            error!(scene = %scene_path.display(), "{err}");
            ExitCode::from(3)
        }
    }
}

fn default_output_path(scene_path: &Path) -> PathBuf {
    if scene_path.extension().is_some() {
        let mut output = scene_path.to_path_buf();
        output.set_extension("png");
        output
    } else {
        let mut output: OsString = scene_path.as_os_str().to_os_string();
        output.push(".png");
        PathBuf::from(output)
    }
}

fn run_bench(
    scene: Option<PathBuf>,
    width: Option<u32>,
    height: Option<u32>,
    iterations: u32,
    warmup: u32,
    cfg: &AppConfig,
) -> ExitCode {
    if iterations == 0 {
        error!("iterations must be greater than zero");
        return ExitCode::from(2);
    }

    match resolve_scene_path(scene, cfg) {
        Ok(scene_path) => match load_and_eval_scene(&scene_path) {
            Ok(state) => {
                let scene_settings = extract_scene_render_settings(&state);
                let options = merged_render_options(&scene_settings, width, height);
                info!(
                    scene = %scene_path.display(),
                    width = options.width,
                    height = options.height,
                    iterations,
                    warmup,
                    "starting benchmark"
                );

                let modes = [AccelMode::Naive, AccelMode::Bvh, AccelMode::Bricks];
                let mut rows: Vec<(AccelMode, f64, f64, u128, u128)> = Vec::new();

                for mode in modes {
                    let bench_output = std::env::temp_dir()
                        .join(format!("ftc-bench-{}-{mode:?}.png", std::process::id()));

                    for _ in 0..warmup {
                        if let Err(err) =
                            render_depth_png_with_accel(&state, &bench_output, options, mode)
                        {
                            error!(accel = ?mode, "{err}");
                            return ExitCode::from(4);
                        }
                    }

                    let mut total_secs = 0.0_f64;
                    let mut best_ms = u128::MAX;
                    let mut worst_ms = 0_u128;
                    for _ in 0..iterations {
                        let start = Instant::now();
                        if let Err(err) =
                            render_depth_png_with_accel(&state, &bench_output, options, mode)
                        {
                            error!(accel = ?mode, "{err}");
                            return ExitCode::from(4);
                        }
                        let elapsed = start.elapsed();
                        let elapsed_ms = elapsed.as_millis();
                        total_secs += elapsed.as_secs_f64();
                        best_ms = best_ms.min(elapsed_ms);
                        worst_ms = worst_ms.max(elapsed_ms);
                    }

                    let _ = std::fs::remove_file(&bench_output);
                    let avg_ms = (total_secs * 1000.0) / f64::from(iterations);
                    let megapixels =
                        (f64::from(options.width) * f64::from(options.height)) / 1_000_000.0;
                    let mpix_per_sec = megapixels / (total_secs / f64::from(iterations));
                    rows.push((mode, avg_ms, mpix_per_sec, best_ms, worst_ms));
                }

                info!("benchmark results:");
                for (mode, avg_ms, mpix_per_sec, best_ms, worst_ms) in rows {
                    info!(
                        accel = ?mode,
                        avg_ms = avg_ms,
                        best_ms,
                        worst_ms,
                        mpix_per_sec = mpix_per_sec,
                        "benchmark_result"
                    );
                }
                ExitCode::SUCCESS
            }
            Err(err) => {
                error!(scene = %scene_path.display(), "{err}");
                ExitCode::from(3)
            }
        },
        Err(CoreError::MissingSceneInput) => {
            error!("missing scene input; pass --scene <path> or set FORGEDTHOUGHTS_SCENE");
            ExitCode::from(2)
        }
        Err(err) => {
            error!("{err}");
            ExitCode::from(3)
        }
    }
}

fn run_list(kind: ListCommand) -> ExitCode {
    let category = match kind {
        ListCommand::Materials => BuiltinLibraryCategory::Materials,
        ListCommand::Objects => BuiltinLibraryCategory::Objects,
        ListCommand::Skeletons => BuiltinLibraryCategory::Skeletons,
        ListCommand::Scenes => BuiltinLibraryCategory::Scenes,
    };

    for item in builtin_library_items(Some(category)) {
        let metadata = builtin_library_item_metadata(&item);
        let tags = if metadata.tags.is_empty() {
            String::new()
        } else {
            format!(" [{}]", metadata.tags.join(", "))
        };
        println!("{}\t{}", metadata.name, item.path);
        println!("  {}{}", metadata.description, tags);
        if !metadata.params.is_empty() {
            println!("  params:");
            for param in &metadata.params {
                let mut extras = Vec::new();
                if let Some(default) = &param.default {
                    extras.push(format!("default={default}"));
                }
                if let Some(min) = &param.min {
                    extras.push(format!("min={min}"));
                }
                if let Some(max) = &param.max {
                    extras.push(format!("max={max}"));
                }
                let extras = if extras.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", extras.join(", "))
                };
                println!("    - {}: {}{}", param.name, param.param_type, extras);
                if let Some(description) = &param.description {
                    println!("      {}", description);
                }
            }
        }
    }

    ExitCode::SUCCESS
}

fn run_ray(params: RayParams, cfg: &AppConfig) -> ExitCode {
    run_with_watch(params.scene.clone(), params.watch, cfg, |scene_path| {
        run_ray_once(scene_path, &params)
    })
}

fn run_ray_once(scene_path: &Path, params: &RayParams) -> ExitCode {
    match load_and_eval_scene(scene_path) {
        Ok(state) => {
            let scene_settings = extract_scene_render_settings(&state);
            let options = merged_render_options(&scene_settings, params.width, params.height);
            let accel = params
                .accel
                .or(scene_settings.accel)
                .unwrap_or(AccelMode::Bvh);
            let output_path = params
                .output
                .as_deref()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| default_output_path(scene_path));
            let tile_size = params.tile_size.max(8);
            let tiles_x = options.width.div_ceil(tile_size);
            let tiles_y = options.height.div_ceil(tile_size);
            let tiles_total = u64::from(tiles_x) * u64::from(tiles_y);
            let progress = ProgressBar::new(tiles_total.max(1));
            let style = ProgressStyle::with_template(
                "[{elapsed_precise}] {wide_bar} {pos}/{len} tiles {msg}",
            )
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("=>-");
            progress.set_style(style);

            let render_start = Instant::now();
            let image = match render_ray_progressive_with_accel(
                &state,
                options,
                accel,
                RaySettings {
                    max_depth: params.depth.max(1),
                    tile_size,
                    aa_samples: params.aa.max(1),
                    debug_aov: params.debug_aov,
                },
                |step, image| {
                    progress.set_position(u64::from(step.tiles_done));
                    progress.set_message(format!("{} ms", step.elapsed_ms));
                    image.save(&output_path)?;
                    Ok(())
                },
            ) {
                Ok(image) => image,
                Err(err) => {
                    progress.abandon_with_message("failed");
                    error!(output = %output_path.display(), "{err}");
                    return ExitCode::from(4);
                }
            };
            if let Err(err) = image.save(&output_path) {
                progress.abandon_with_message("failed");
                error!(output = %output_path.display(), "{err}");
                return ExitCode::from(4);
            }
            progress.finish_with_message("done");
            let render_elapsed = render_start.elapsed();
            let render_secs = render_elapsed.as_secs_f64();
            let render_secs_display = format!("{render_secs:.3}s");
            let megapixels = (f64::from(options.width) * f64::from(options.height)) / 1_000_000.0;
            let mpix_per_sec = megapixels / render_secs.max(f64::EPSILON);
            let mpix_per_sec_display = format!("{mpix_per_sec:.3}");
            info!(
                output = %output_path.display(),
                width = options.width,
                height = options.height,
                accel = ?accel,
                depth = params.depth.max(1),
                tile_size,
                aa = params.aa.max(1),
                debug_aov = ?params.debug_aov,
                "trace rendered"
            );
            info!(
                render_secs = %render_secs_display,
                mpix_per_sec = %mpix_per_sec_display,
                "benchmark"
            );
            ExitCode::SUCCESS
        }
        Err(err) => {
            error!(scene = %scene_path.display(), "{err}");
            ExitCode::from(3)
        }
    }
}

struct RenderParams {
    scene: Option<PathBuf>,
    output: Option<PathBuf>,
    width: Option<u32>,
    height: Option<u32>,
    accel: Option<AccelMode>,
    tile_size: u32,
    aa: u32,
    watch: bool,
}

struct RayParams {
    scene: Option<PathBuf>,
    output: Option<PathBuf>,
    width: Option<u32>,
    height: Option<u32>,
    accel: Option<AccelMode>,
    depth: u32,
    tile_size: u32,
    aa: u32,
    debug_aov: Option<RayDebugAov>,
    watch: bool,
}

fn run_with_watch(
    scene: Option<PathBuf>,
    watch: bool,
    cfg: &AppConfig,
    mut action: impl FnMut(&Path) -> ExitCode,
) -> ExitCode {
    match resolve_scene_path(scene, cfg) {
        Ok(scene_path) => {
            if !watch {
                return action(&scene_path);
            }

            let mut last_stamp = file_stamp(&scene_path);
            info!(scene = %scene_path.display(), "watch mode active");
            let _ = action(&scene_path);

            loop {
                thread::sleep(Duration::from_millis(250));
                let current_stamp = file_stamp(&scene_path);
                if current_stamp.is_some() && current_stamp != last_stamp {
                    last_stamp = current_stamp;
                    info!(scene = %scene_path.display(), "change detected, rerunning");
                    let _ = action(&scene_path);
                }
            }
        }
        Err(CoreError::MissingSceneInput) => {
            error!("missing scene input; pass --scene <path> or set FORGEDTHOUGHTS_SCENE");
            ExitCode::from(2)
        }
        Err(err) => {
            error!("{err}");
            ExitCode::from(3)
        }
    }
}

fn file_stamp(path: &Path) -> Option<(u64, u128)> {
    let metadata = fs::metadata(path).ok()?;
    let modified = metadata.modified().ok()?;
    Some((metadata.len(), modified_millis(modified)))
}

fn modified_millis(time: SystemTime) -> u128 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn merged_render_options(
    scene_settings: &SceneRenderSettings,
    cli_width: Option<u32>,
    cli_height: Option<u32>,
) -> RenderOptions {
    let mut options = RenderOptions::default();
    if let Some(v) = scene_settings.width {
        options.width = v;
    }
    if let Some(v) = scene_settings.height {
        options.height = v;
    }
    if let Some(v) = scene_settings.max_steps {
        options.max_steps = v;
    }
    if let Some(v) = scene_settings.max_dist {
        options.max_dist = v;
    }
    if let Some(v) = scene_settings.epsilon {
        options.epsilon = v;
    }
    if let Some(v) = scene_settings.step_scale {
        options.step_scale = v.clamp(0.05, 1.0);
    }
    if let Some(v) = scene_settings.camera_z {
        options.camera_z = v;
    }
    if let Some(v) = scene_settings.fov_y_degrees {
        options.fov_y_degrees = v;
    }
    if let Some(v) = cli_width {
        options.width = v;
    }
    if let Some(v) = cli_height {
        options.height = v;
    }
    options
}
