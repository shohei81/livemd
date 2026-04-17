mod app;
mod audio;
mod config;
mod filter;
mod markdown;
mod msg;
mod transcribe;
mod translate;
mod ui;
mod vad;

use anyhow::Result;
use clap::Parser;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(version, about = "Live bilingual voice transcription TUI")]
struct Args {
    /// Output markdown file. Example: `livemd notes.md`
    #[arg(value_name = "OUTPUT.md")]
    output: Option<PathBuf>,

    /// Path to config file (default: ./livemd.toml or ~/.config/livemd/livemd.toml)
    #[arg(short = 'c', long)]
    config: Option<PathBuf>,

    /// Override whisper model path
    #[arg(short = 'm', long)]
    model: Option<PathBuf>,

    /// Override starting language (en | ja | auto)
    #[arg(short = 'l', long)]
    language: Option<String>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let log_dir = log_dir_path();
    std::fs::create_dir_all(&log_dir).ok();
    init_logging(&log_dir)?;

    let mut cfg = config::Config::load(args.config.as_deref())?;
    if let Some(m) = args.model {
        cfg.model_path = m;
    }
    if let Some(o) = args.output {
        cfg.output_path = o;
    }
    if let Some(l) = args.language {
        cfg.language = l;
    }
    cfg.log_dir = log_dir;

    app::run(cfg)
}

fn log_dir_path() -> PathBuf {
    if let Ok(v) = std::env::var("LIVEMD_LOG_DIR") {
        return PathBuf::from(v);
    }
    dirs::home_dir()
        .map(|h| h.join(".config/livemd/logs"))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn init_logging(preferred: &Path) -> Result<()> {
    use tracing_subscriber::{fmt, EnvFilter};

    let log_path = if preferred.exists() {
        preferred.join("livemd.log")
    } else {
        PathBuf::from("livemd.log")
    };
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(file)
        .with_ansi(false)
        .init();
    Ok(())
}
