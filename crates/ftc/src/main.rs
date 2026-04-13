use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use clap::Parser;
use forgedthoughts::{NodeRenderSettings, load_and_eval_scene, render_node_png};
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

    /// Image width in pixels
    #[arg(long, default_value_t = 512)]
    width: u32,

    /// Image height in pixels
    #[arg(long, default_value_t = 512)]
    height: u32,

    /// World-space coordinate range (pixels map to [0, world_size])
    #[arg(long, default_value_t = 1.0)]
    world_size: f32,

    /// Tile size for rendering
    #[arg(long, default_value_t = 64)]
    tile_size: u32,

    /// Re-render when the graph file changes
    #[arg(long)]
    watch: bool,

    /// Increase log output (-v, -vv)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
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
    let state = match load_and_eval_scene(graph_path) {
        Ok(state) => state,
        Err(err) => {
            error!(graph = %graph_path.display(), "{err}");
            return ExitCode::from(3);
        }
    };

    let output_path = cli
        .output
        .clone()
        .unwrap_or_else(|| default_output_path(graph_path));

    let settings = NodeRenderSettings {
        width: cli.width.max(1),
        height: cli.height.max(1),
        tile_size: cli.tile_size.max(8),
        world_size: if cli.world_size > 0.0 {
            cli.world_size
        } else {
            1.0
        },
    };

    let tiles_x = settings.width.div_ceil(settings.tile_size);
    let tiles_y = settings.height.div_ceil(settings.tile_size);
    let tiles_total = u64::from(tiles_x) * u64::from(tiles_y);
    let progress = ProgressBar::new(tiles_total.max(1));
    let style =
        ProgressStyle::with_template("[{elapsed_precise}] {wide_bar} {pos}/{len} tiles {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("=>-");
    progress.set_style(style);

    let render_start = Instant::now();
    let image = match render_node_png(&state, settings, |step, img| {
        progress.set_position(u64::from(step.tiles_done));
        progress.set_message(format!("{} ms", step.elapsed_ms));
        img.save(&output_path)?;
        Ok(())
    }) {
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
    info!(
        output = %output_path.display(),
        width = settings.width,
        height = settings.height,
        elapsed_ms = render_start.elapsed().as_millis(),
        "graph rendered"
    );
    ExitCode::SUCCESS
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
