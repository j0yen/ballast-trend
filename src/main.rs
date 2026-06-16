use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod snapshot;
mod report;
mod schema;

#[derive(Parser)]
#[command(name = "ballast-trend", about = "Disk growth rate tracker: derivatives of ballast-survey snapshots")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Capture a ballast-survey --json snapshot to the trend ring
    Snapshot {
        /// Override the current time (RFC3339) for deterministic testing
        #[arg(long)]
        now: Option<String>,

        /// Maximum number of snapshots to keep in the ring
        #[arg(long, default_value = "30")]
        keep: usize,

        /// Directory to store snapshots (default: ~/.local/state/ballast/trend)
        #[arg(long)]
        state_dir: Option<PathBuf>,
    },
    /// Report per-path growth rates from the two most recent snapshots
    Report {
        /// Output JSON instead of a human-readable table
        #[arg(long)]
        json: bool,

        /// High-water mark percentage for ETA projection (default: 95)
        #[arg(long, default_value = "95.0")]
        high_water_pct: f64,

        /// Override the current time (RFC3339) for deterministic testing
        #[arg(long)]
        now: Option<String>,

        /// Directory to read snapshots from (default: ~/.local/state/ballast/trend)
        #[arg(long)]
        state_dir: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Snapshot { now, keep, state_dir } => {
            let state_dir = resolve_state_dir(state_dir)?;
            snapshot::run(&state_dir, now.as_deref(), keep)
        }
        Commands::Report { json, high_water_pct, now, state_dir } => {
            let state_dir = resolve_state_dir(state_dir)?;
            report::run(&state_dir, json, high_water_pct, now.as_deref())
        }
    }
}

fn resolve_state_dir(override_path: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(p) = override_path {
        return Ok(p);
    }
    let home = std::env::var("HOME").context("HOME not set")?;
    Ok(PathBuf::from(home).join(".local/state/ballast/trend"))
}
