/// Types mirroring the ballast-survey --json output schema.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SurveyOutput {
    pub summary: SurveySummary,
    pub entries: Vec<SurveyEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SurveySummary {
    pub reclaimable_bytes: u64,
    pub entry_count: usize,
    pub scanned_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SurveyEntry {
    pub path: String,
    pub kind: String,
    pub bytes: u64,
    pub entries: u64,
    pub mtime: String,
    pub age_days: f64,
    // Optional fields present on rust-target entries
    #[serde(default)]
    pub crate_name: Option<String>,
    #[serde(default)]
    pub cloud_info: Option<serde_json::Value>,
}
