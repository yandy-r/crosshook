use crate::profile::GameProfile;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LutrisImportPreview {
    pub entries: Vec<LutrisImportEntry>,
    pub lutris_root: Option<PathBuf>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LutrisImportEntry {
    pub source_path: PathBuf,
    pub suggested_name: String,
    pub game_name: String,
    pub runner: String,
    pub mapped: GameProfile,
    pub warnings: Vec<String>,
    pub importable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LutrisImportOutcome {
    Imported,
    Skipped,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LutrisImportEntryResult {
    pub entry: LutrisImportEntry,
    pub outcome: LutrisImportOutcome,
    pub profile_name: Option<String>,
    pub profile_path: Option<PathBuf>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LutrisImportResult {
    pub results: Vec<LutrisImportEntryResult>,
    pub imported_count: usize,
    pub skipped_count: usize,
    pub failed_count: usize,
}
