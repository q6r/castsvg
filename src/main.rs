mod cast;
mod record;
mod svg;
mod term;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Record terminal sessions and render them as self-contained animated SVG.
#[derive(Parser)]
#[command(name = "castsvg", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Render an asciinema .cast file into an animated SVG.
    Render(RenderArgs),
    /// Record a terminal session into an asciinema .cast file.
    Record(RecordArgs),
}

#[derive(Parser)]
struct RenderArgs {
    /// Input .cast file (asciicast v2).
    input: PathBuf,
    /// Output .svg file.
    #[arg(short, long, default_value = "out.svg")]
    output: PathBuf,
    /// Font size in pixels.
    #[arg(long, default_value_t = 14.0)]
    font_size: f64,
    /// Colour theme: `dark` or `light`.
    #[arg(long, default_value = "dark")]
    theme: String,
    /// Coalesce output bursts closer together than this many milliseconds.
    #[arg(long, default_value_t = 40.0)]
    min_frame_ms: f64,
    /// Cap idle pauses to at most this many milliseconds.
    #[arg(long, default_value_t = 1000.0)]
    idle_cap_ms: f64,
    /// Hold the final frame this long before looping.
    #[arg(long, default_value_t = 1500.0)]
    end_pause_ms: f64,
    /// Play once and stop on the last frame instead of looping forever.
    #[arg(long)]
    no_loop: bool,
}

#[derive(Parser)]
struct RecordArgs {
    /// Output .cast file.
    output: PathBuf,
    /// Command to run (defaults to your shell).
    #[arg(short, long)]
    command: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Render(args) => render(args),
        Commands::Record(args) => record::run(&args.output, args.command.as_deref()),
    }
}

fn render(args: RenderArgs) -> Result<()> {
    let theme = svg::Theme::from_name(&args.theme)
        .with_context(|| format!("unknown theme '{}' (try 'dark' or 'light')", args.theme))?;

    let cast = cast::Cast::parse(&args.input)?;
    let model = term::build_model(
        &cast,
        args.min_frame_ms,
        args.idle_cap_ms,
        args.end_pause_ms,
    );

    let opts = svg::Options {
        font_size: args.font_size,
        theme,
        looping: !args.no_loop,
    };
    let out = svg::render(&model, &opts);
    std::fs::write(&args.output, &out)
        .with_context(|| format!("writing {}", args.output.display()))?;

    eprintln!(
        "castsvg: {} frames, {}x{} cells -> {} ({} KiB)",
        model.frames.len(),
        model.cols,
        model.rows,
        args.output.display(),
        out.len() / 1024
    );
    Ok(())
}
