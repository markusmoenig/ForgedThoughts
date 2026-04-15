use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use clap::Parser;
use forgedthoughts::{
    GraphCamera, GraphSceneSettings, GraphSky, GraphSun, NodeRenderSettings,
    graph_render_source_kind, load_and_eval_scene, load_graph_file, render_node_png,
    render_terrain_png,
};
use indicatif::{ProgressBar, ProgressStyle};
use tracing::{error, info, level_filters::LevelFilter};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(name = "ftc")]
#[command(about = "Render a ForgedThoughts TOML graph to a PNG.")]
struct Cli {
    /// Path to a TOML graph file
    graph: PathBuf,

    /// Output PNG path (default: <graph>.png)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Image width in pixels (overrides TOML)
    #[arg(long)]
    width: Option<u32>,

    /// Image height in pixels (overrides TOML)
    #[arg(long)]
    height: Option<u32>,

    /// Tile size for rendering
    #[arg(long, default_value_t = 64)]
    tile_size: u32,

    /// Re-render when the graph file changes
    #[arg(long)]
    watch: bool,

    /// Increase log output (-v, -vv)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Render material lanes on a close-up preview sphere (Substance-style debug view)
    #[arg(long)]
    material_preview: bool,

    /// Material debug visualization mode: off, mask, lanes
    #[arg(long, value_parser = ["off", "mask", "lanes"])]
    material_debug: Option<String>,
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
    run(cli)
}

fn run(cli: Cli) -> ExitCode {
    if !is_toml_graph(&cli.graph) {
        error!(
            graph = %cli.graph.display(),
            "graphs must be TOML files"
        );
        return ExitCode::from(2);
    }

    if !cli.watch {
        return render_graph_once(&cli.graph, &cli);
    }

    let mut last_stamp = file_stamp(&cli.graph);
    info!(graph = %cli.graph.display(), "watch mode active");
    let _ = render_graph_once(&cli.graph, &cli);

    loop {
        thread::sleep(Duration::from_millis(250));
        let current_stamp = file_stamp(&cli.graph);
        if current_stamp.is_some() && current_stamp != last_stamp {
            last_stamp = current_stamp;
            info!(graph = %cli.graph.display(), "change detected, rerendering");
            let _ = render_graph_once(&cli.graph, &cli);
        }
    }
}

fn render_graph_once(graph_path: &Path, cli: &Cli) -> ExitCode {
    // Load the raw graph file first so we can read stage/target/camera/etc.
    let graph = match load_graph_file(graph_path) {
        Ok(g) => g,
        Err(err) => {
            error!(graph = %graph_path.display(), "failed to parse graph: {err}");
            return ExitCode::from(3);
        }
    };

    // Load and JIT-compile the scene (node eval state).
    let state = match load_and_eval_scene(graph_path) {
        Ok(s) => s,
        Err(err) => {
            error!(graph = %graph_path.display(), "{err}");
            return ExitCode::from(3);
        }
    };

    let output_path = cli
        .output
        .clone()
        .unwrap_or_else(|| default_output_path(graph_path));

    // Resolve width/height: CLI flag > TOML > default 512
    let width = cli
        .width
        .or(graph.render.width)
        .unwrap_or(512)
        .max(1);
    let height = cli
        .height
        .or(graph.render.height)
        .unwrap_or(512)
        .max(1);
    let tile_size = cli.tile_size.max(8);

    let stage = graph.render.stage.as_str();
    let target = graph.render.target.as_str();
    let source_kind = match graph_render_source_kind(&graph) {
        Ok(kind) => kind,
        Err(err) => {
            error!(graph = %graph_path.display(), "{err}");
            return ExitCode::from(3);
        }
    };

    match (stage, target) {
        ("scene", "raytrace") => {
            render_terrain(
                graph_path,
                &state,
                source_kind,
                &graph.camera,
                &graph.sun,
                &graph.sky,
                &graph.scene,
                cli.material_preview,
                cli.material_debug.as_deref(),
                width,
                height,
                tile_size,
                &output_path,
            )
        }
        _ => {
            // Default: height → grayscale node render
            render_node(
                graph_path,
                &state,
                source_kind,
                graph.scene.world_size,
                width,
                height,
                tile_size,
                &output_path,
            )
        }
    }
}

fn render_node(
    _graph_path: &Path,
    state: &forgedthoughts::EvalState,
    source_kind: forgedthoughts::GraphRenderSourceKind,
    world_size: f32,
    width: u32,
    height: u32,
    tile_size: u32,
    output_path: &Path,
) -> ExitCode {
    let settings = NodeRenderSettings {
        width,
        height,
        tile_size,
        world_size: world_size.max(f32::EPSILON),
    };

    let tiles_x = settings.width.div_ceil(settings.tile_size);
    let tiles_y = settings.height.div_ceil(settings.tile_size);
    let tiles_total = u64::from(tiles_x) * u64::from(tiles_y);
    let progress = make_progress_bar(tiles_total);

    let render_start = Instant::now();
    let image = match render_node_png(state, source_kind, settings, |step, img| {
        progress.set_position(u64::from(step.tiles_done));
        progress.set_message(format!("{} ms", step.elapsed_ms));
        img.save(output_path)?;
        Ok(())
    }) {
        Ok(image) => image,
        Err(err) => {
            progress.abandon_with_message("failed");
            error!(output = %output_path.display(), "{err}");
            return ExitCode::from(4);
        }
    };

    if let Err(err) = image.save(output_path) {
        progress.abandon_with_message("failed");
        error!(output = %output_path.display(), "{err}");
        return ExitCode::from(4);
    }

    progress.finish_with_message("done");
    info!(
        output = %output_path.display(),
        width,
        height,
        elapsed_ms = render_start.elapsed().as_millis(),
        "node render complete"
    );
    ExitCode::SUCCESS
}

#[allow(clippy::too_many_arguments)]
fn render_terrain(
    graph_path: &Path,
    state: &forgedthoughts::EvalState,
    source_kind: forgedthoughts::GraphRenderSourceKind,
    camera: &GraphCamera,
    sun: &GraphSun,
    sky: &GraphSky,
    scene: &GraphSceneSettings,
    material_preview: bool,
    material_debug: Option<&str>,
    width: u32,
    height: u32,
    tile_size: u32,
    output_path: &Path,
) -> ExitCode {
    let tiles_x = width.div_ceil(tile_size);
    let tiles_y = height.div_ceil(tile_size);
    let tiles_total = u64::from(tiles_x) * u64::from(tiles_y);
    let progress = make_progress_bar(tiles_total);

    let render_start = Instant::now();
    let image = match render_terrain_png(
        state,
        source_kind,
        width,
        height,
        tile_size,
        camera,
        sun,
        sky,
        scene,
        material_preview,
        material_debug,
        |step, img| {
            progress.set_position(u64::from(step.tiles_done));
            progress.set_message(format!("{} ms", step.elapsed_ms));
            img.save(output_path)?;
            Ok(())
        },
    ) {
        Ok(image) => image,
        Err(err) => {
            progress.abandon_with_message("failed");
            error!(graph = %graph_path.display(), output = %output_path.display(), "{err}");
            return ExitCode::from(4);
        }
    };

    if let Err(err) = image.save(output_path) {
        progress.abandon_with_message("failed");
        error!(output = %output_path.display(), "{err}");
        return ExitCode::from(4);
    }

    progress.finish_with_message("done");
    info!(
        output = %output_path.display(),
        width,
        height,
        elapsed_ms = render_start.elapsed().as_millis(),
        "terrain render complete"
    );
    ExitCode::SUCCESS
}

fn make_progress_bar(tiles_total: u64) -> ProgressBar {
    let progress = ProgressBar::new(tiles_total.max(1));
    let style =
        ProgressStyle::with_template("[{elapsed_precise}] {wide_bar} {pos}/{len} tiles {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("=>-");
    progress.set_style(style);
    progress
}

fn is_toml_graph(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("toml"))
}

fn default_output_path(graph_path: &Path) -> PathBuf {
    if graph_path.extension().is_some() {
        let mut output = graph_path.to_path_buf();
        output.set_extension("png");
        output
    } else {
        let mut output: OsString = graph_path.as_os_str().to_os_string();
        output.push(".png");
        PathBuf::from(output)
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
