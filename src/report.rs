use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

use crate::snapshot::{list_snapshots, load_snapshot, TrendSnapshot};

/// Per-path growth entry in the report.
#[derive(Debug, Serialize)]
pub struct PathEntry {
    pub path: String,
    pub old_bytes: u64,
    pub new_bytes: u64,
    pub delta_bytes: i64,
    pub bytes_per_day: Option<f64>,
    pub status: PathStatus,
    pub eta_days: Option<f64>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PathStatus {
    Growing,
    Shrinking,
    Stable,
    New,
    Removed,
}

#[derive(Debug, Serialize)]
pub struct ReportOutput {
    pub old_snapshot_at: String,
    pub new_snapshot_at: String,
    pub interval_hours: f64,
    pub free_bytes: Option<u64>,
    pub entries: Vec<PathEntry>,
    pub high_water_pct: f64,
}

/// Compute the report from the two most recent snapshots.
pub fn run(
    state_dir: &Path,
    json_output: bool,
    high_water_pct: f64,
    _now_override: Option<&str>,
) -> Result<()> {
    let snapshots = list_snapshots(state_dir)?;
    if snapshots.len() < 2 {
        bail!(
            "Need at least 2 snapshots in {} (found {}). Run `ballast-trend snapshot` first.",
            state_dir.display(),
            snapshots.len()
        );
    }

    let old_path = &snapshots[snapshots.len() - 2];
    let new_path = &snapshots[snapshots.len() - 1];

    let old_snap = load_snapshot(old_path)
        .with_context(|| format!("loading old snapshot {}", old_path.display()))?;
    let new_snap = load_snapshot(new_path)
        .with_context(|| format!("loading new snapshot {}", new_path.display()))?;

    let report = compute_report(&old_snap, &new_snap, high_water_pct)?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_human(&report);
    }

    Ok(())
}

pub fn compute_report(
    old: &TrendSnapshot,
    new: &TrendSnapshot,
    high_water_pct: f64,
) -> Result<ReportOutput> {
    let interval_secs = (new.captured_at - old.captured_at).num_seconds();
    if interval_secs <= 0 {
        bail!("New snapshot is not newer than old snapshot");
    }
    let interval_days = interval_secs as f64 / 86400.0;
    let interval_hours = interval_secs as f64 / 3600.0;

    // Build path → bytes maps
    let old_map: HashMap<&str, u64> = old
        .survey
        .entries
        .iter()
        .map(|e| (e.path.as_str(), e.bytes))
        .collect();
    let new_map: HashMap<&str, u64> = new
        .survey
        .entries
        .iter()
        .map(|e| (e.path.as_str(), e.bytes))
        .collect();

    // Gather all paths
    let mut all_paths: Vec<&str> = old_map.keys().chain(new_map.keys()).copied().collect();
    all_paths.sort_unstable();
    all_paths.dedup();

    let mut entries: Vec<PathEntry> = Vec::new();

    for path in &all_paths {
        let old_bytes = old_map.get(*path).copied();
        let new_bytes = new_map.get(*path).copied();

        let entry = match (old_bytes, new_bytes) {
            (None, Some(nb)) => PathEntry {
                path: path.to_string(),
                old_bytes: 0,
                new_bytes: nb,
                delta_bytes: nb as i64,
                bytes_per_day: None,
                status: PathStatus::New,
                eta_days: None,
            },
            (Some(ob), None) => PathEntry {
                path: path.to_string(),
                old_bytes: ob,
                new_bytes: 0,
                delta_bytes: -(ob as i64),
                bytes_per_day: Some(-(ob as f64) / interval_days),
                status: PathStatus::Removed,
                eta_days: None,
            },
            (Some(ob), Some(nb)) => {
                let delta = nb as i64 - ob as i64;
                let bpd = delta as f64 / interval_days;
                let status = if delta > 0 {
                    PathStatus::Growing
                } else if delta < 0 {
                    PathStatus::Shrinking
                } else {
                    PathStatus::Stable
                };
                PathEntry {
                    path: path.to_string(),
                    old_bytes: ob,
                    new_bytes: nb,
                    delta_bytes: delta,
                    bytes_per_day: Some(bpd),
                    status,
                    eta_days: None, // filled below
                }
            }
            (None, None) => unreachable!(),
        };
        entries.push(entry);
    }

    // Sort by delta_bytes descending (fastest growers first)
    entries.sort_by(|a, b| b.delta_bytes.cmp(&a.delta_bytes));

    // Compute ETA for growing entries using total_bytes from new snapshot
    let total_bytes = new.survey.summary.reclaimable_bytes
        + new
            .survey
            .entries
            .iter()
            .map(|e| e.bytes)
            .sum::<u64>();

    let high_water_bytes = (total_bytes as f64 * high_water_pct / 100.0) as u64;

    // We need free bytes to compute ETA. Estimate from survey's reclaimable + total.
    // Survey reclaimable_bytes = bytes that *can* be freed, not the disk free space.
    // We'll skip ETA if we can't determine disk usage properly, but let's compute
    // a simple "at current rate, how many days to accumulate high_water_pct of current total"
    // For each growing path: ETA = (high_water_bytes - new_bytes) / bpd

    for entry in &mut entries {
        if entry.status == PathStatus::Growing {
            if let Some(bpd) = entry.bytes_per_day {
                if bpd > 0.0 {
                    let room = high_water_bytes.saturating_sub(entry.new_bytes) as f64;
                    entry.eta_days = Some(room / bpd);
                }
            }
        }
    }

    Ok(ReportOutput {
        old_snapshot_at: old.captured_at.to_rfc3339(),
        new_snapshot_at: new.captured_at.to_rfc3339(),
        interval_hours,
        free_bytes: None,
        entries,
        high_water_pct,
    })
}

fn format_bytes(b: i64) -> String {
    let abs = b.unsigned_abs();
    let sign = if b < 0 { "-" } else { "+" };
    if abs >= 1_073_741_824 {
        format!("{}{:.1}G", sign, abs as f64 / 1_073_741_824.0)
    } else if abs >= 1_048_576 {
        format!("{}{:.1}M", sign, abs as f64 / 1_048_576.0)
    } else if abs >= 1024 {
        format!("{}{:.1}K", sign, abs as f64 / 1024.0)
    } else {
        format!("{}{}", sign, abs)
    }
}

fn format_bytes_unsigned(b: u64) -> String {
    if b >= 1_073_741_824 {
        format!("{:.1}G", b as f64 / 1_073_741_824.0)
    } else if b >= 1_048_576 {
        format!("{:.1}M", b as f64 / 1_048_576.0)
    } else if b >= 1024 {
        format!("{:.1}K", b as f64 / 1024.0)
    } else {
        format!("{}", b)
    }
}

fn print_human(report: &ReportOutput) {
    println!("ballast-trend report");
    println!(
        "  old: {}  →  new: {}  ({:.1}h interval)",
        &report.old_snapshot_at[..19],
        &report.new_snapshot_at[..19],
        report.interval_hours
    );
    println!("  high-water mark: {:.0}%", report.high_water_pct);
    println!();
    println!(
        "{:<55} {:>10} {:>12} {:>14} {:>10}",
        "path", "new-size", "delta", "bytes/day", "ETA(days)"
    );
    println!("{}", "-".repeat(105));

    for e in &report.entries {
        let bpd_str = match &e.status {
            PathStatus::New => "new—no rate".to_string(),
            PathStatus::Removed => "removed".to_string(),
            _ => e
                .bytes_per_day
                .map(|v| format_bytes(v as i64))
                .unwrap_or_else(|| "-".to_string()),
        };
        let eta_str = e
            .eta_days
            .map(|d| format!("{:.1}", d))
            .unwrap_or_else(|| "-".to_string());

        let path_display = if e.path.len() > 54 {
            format!("…{}", &e.path[e.path.len() - 53..])
        } else {
            e.path.clone()
        };

        println!(
            "{:<55} {:>10} {:>12} {:>14} {:>10}",
            path_display,
            format_bytes_unsigned(e.new_bytes),
            format_bytes(e.delta_bytes),
            bpd_str,
            eta_str
        );
    }
    println!();
    println!(
        "Note: shrinking paths ({}) are excluded from ETA projection.",
        report
            .entries
            .iter()
            .filter(|e| e.status == PathStatus::Shrinking)
            .count()
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{SurveyEntry, SurveyOutput, SurveySummary};
    use crate::snapshot::TrendSnapshot;

    fn make_snap(at: &str, entries: Vec<(&str, u64)>) -> TrendSnapshot {
        let survey_entries: Vec<SurveyEntry> = entries
            .into_iter()
            .map(|(path, bytes)| SurveyEntry {
                path: path.to_string(),
                kind: "dir".to_string(),
                bytes,
                entries: 1,
                mtime: at.to_string(),
                age_days: 0.0,
                crate_name: None,
                cloud_info: None,
            })
            .collect();
        let total: u64 = survey_entries.iter().map(|e| e.bytes).sum();
        TrendSnapshot {
            captured_at: at.parse().unwrap(),
            survey: SurveyOutput {
                summary: SurveySummary {
                    reclaimable_bytes: total,
                    entry_count: survey_entries.len(),
                    scanned_at: at.to_string(),
                },
                entries: survey_entries,
            },
        }
    }

    #[test]
    fn test_bytes_per_day_rate() {
        // 24h interval, path grows 1G → rate = 1G/day
        let old = make_snap(
            "2026-01-01T00:00:00Z",
            vec![("/home/foo/target", 1_000_000_000)],
        );
        let new = make_snap(
            "2026-01-02T00:00:00Z",
            vec![("/home/foo/target", 2_000_000_000)],
        );
        let report = compute_report(&old, &new, 95.0).unwrap();
        let e = report.entries.iter().find(|e| e.path == "/home/foo/target").unwrap();
        assert_eq!(e.status, PathStatus::Growing);
        let bpd = e.bytes_per_day.unwrap();
        assert!((bpd - 1_000_000_000.0).abs() < 1.0, "bpd={bpd}");
    }

    #[test]
    fn test_new_path_no_rate() {
        let old = make_snap("2026-01-01T00:00:00Z", vec![]);
        let new = make_snap(
            "2026-01-02T00:00:00Z",
            vec![("/home/foo/new-dir", 500_000_000)],
        );
        let report = compute_report(&old, &new, 95.0).unwrap();
        let e = report.entries.iter().find(|e| e.path == "/home/foo/new-dir").unwrap();
        assert_eq!(e.status, PathStatus::New);
        assert!(e.bytes_per_day.is_none(), "new path must have no rate");
    }

    #[test]
    fn test_shrinking_excluded_from_eta() {
        let old = make_snap(
            "2026-01-01T00:00:00Z",
            vec![("/big/target", 10_000_000_000)],
        );
        let new = make_snap(
            "2026-01-02T00:00:00Z",
            vec![("/big/target", 5_000_000_000)],
        );
        let report = compute_report(&old, &new, 95.0).unwrap();
        let e = report.entries.iter().find(|e| e.path == "/big/target").unwrap();
        assert_eq!(e.status, PathStatus::Shrinking);
        assert!(e.eta_days.is_none(), "shrinking path must not have ETA");
    }

    #[test]
    fn test_interval_math_12h() {
        // 12h interval, path grows 500MB → rate = 1G/day
        let old = make_snap(
            "2026-01-01T00:00:00Z",
            vec![("/foo/bar", 1_000_000_000)],
        );
        let new = make_snap(
            "2026-01-01T12:00:00Z",
            vec![("/foo/bar", 1_500_000_000)],
        );
        let report = compute_report(&old, &new, 95.0).unwrap();
        let e = report.entries.iter().find(|e| e.path == "/foo/bar").unwrap();
        let bpd = e.bytes_per_day.unwrap();
        // 500MB / 0.5 days = 1G/day
        assert!((bpd - 1_000_000_000.0).abs() < 1.0, "bpd={bpd}");
    }

    #[test]
    fn test_json_output_roundtrip() {
        let old = make_snap(
            "2026-01-01T00:00:00Z",
            vec![("/foo/target", 1_000_000_000)],
        );
        let new = make_snap(
            "2026-01-02T00:00:00Z",
            vec![("/foo/target", 2_000_000_000)],
        );
        let report = compute_report(&old, &new, 95.0).unwrap();
        let json_str = serde_json::to_string(&report).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert!(parsed["entries"].is_array());
        assert_eq!(parsed["entries"][0]["path"], "/foo/target");
    }
}
