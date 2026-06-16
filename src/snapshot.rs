use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::Path;

use crate::schema::SurveyOutput;

/// A single stored snapshot: survey output + capture timestamp.
#[derive(Debug, Serialize, Deserialize)]
pub struct TrendSnapshot {
    pub captured_at: DateTime<Utc>,
    pub survey: SurveyOutput,
}

pub fn run(state_dir: &Path, now_override: Option<&str>, keep: usize) -> Result<()> {
    std::fs::create_dir_all(state_dir)
        .with_context(|| format!("creating state dir {}", state_dir.display()))?;

    // Read survey JSON from stdin
    let mut buf = String::new();
    std::io::stdin()
        .read_to_string(&mut buf)
        .context("reading ballast-survey JSON from stdin")?;

    let survey: SurveyOutput =
        serde_json::from_str(&buf).context("parsing ballast-survey JSON")?;

    let captured_at: DateTime<Utc> = if let Some(s) = now_override {
        s.parse::<DateTime<Utc>>()
            .with_context(|| format!("parsing --now value '{s}'"))?
    } else {
        Utc::now()
    };

    let snap = TrendSnapshot {
        captured_at,
        survey,
    };

    let filename = format!("{}.json", captured_at.format("%Y%m%dT%H%M%SZ"));
    let path = state_dir.join(&filename);
    let json = serde_json::to_string_pretty(&snap).context("serializing snapshot")?;
    std::fs::write(&path, json)
        .with_context(|| format!("writing snapshot to {}", path.display()))?;

    eprintln!("Snapshot saved: {}", path.display());

    prune_ring(state_dir, keep)?;

    Ok(())
}

/// Remove oldest snapshots so at most `keep` remain.
pub fn prune_ring(state_dir: &Path, keep: usize) -> Result<()> {
    let mut files = list_snapshots(state_dir)?;
    if files.len() <= keep {
        return Ok(());
    }
    // list_snapshots returns oldest-first; remove from the front
    let to_remove = files.len() - keep;
    for f in files.drain(..to_remove) {
        std::fs::remove_file(&f)
            .with_context(|| format!("removing old snapshot {}", f.display()))?;
    }
    Ok(())
}

/// Return snapshot paths sorted oldest-first.
pub fn list_snapshots(state_dir: &Path) -> Result<Vec<std::path::PathBuf>> {
    if !state_dir.exists() {
        return Ok(vec![]);
    }
    let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(state_dir)
        .with_context(|| format!("reading state dir {}", state_dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json"))
        .collect();
    paths.sort();
    Ok(paths)
}

pub fn load_snapshot(path: &Path) -> Result<TrendSnapshot> {
    let data = std::fs::read_to_string(path)
        .with_context(|| format!("reading snapshot {}", path.display()))?;
    serde_json::from_str(&data).with_context(|| format!("parsing snapshot {}", path.display()))
}
