use serde::{Deserialize, Serialize};

/// Which profile field contains the stale Proton path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtonPathField {
    /// `steam.proton_path` — used by `steam_applaunch` method.
    SteamProtonPath,
    /// `runtime.proton_path` — used by `proton_run` method.
    RuntimeProtonPath,
}

/// A single migration suggestion for one profile field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationSuggestion {
    pub profile_name: String,
    pub field: ProtonPathField,
    pub old_path: String,
    pub new_path: String,
    pub old_proton_name: String,
    pub new_proton_name: String,
    /// Confidence score: 0.0..=1.0
    pub confidence: f64,
    pub proton_family: String,
    /// True when the suggestion crosses a major version boundary (e.g., 9→10).
    pub crosses_major_version: bool,
}

/// A profile with a stale Proton path that has no matching replacement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnmatchedProfile {
    pub profile_name: String,
    pub field: ProtonPathField,
    pub stale_path: String,
    pub stale_proton_name: String,
}

/// Lightweight Proton install info for frontend display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtonInstallInfo {
    pub name: String,
    /// Executable path (e.g., `.../GE-Proton9-7/proton`), same as `ProtonInstall.path`.
    pub path: String,
    pub is_official: bool,
}

/// Result of scanning all profiles for migration candidates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationScanResult {
    pub suggestions: Vec<MigrationSuggestion>,
    pub unmatched: Vec<UnmatchedProfile>,
    pub profiles_scanned: usize,
    pub affected_count: usize,
    pub installed_proton_versions: Vec<ProtonInstallInfo>,
    pub diagnostics: Vec<String>,
}

/// Outcome of applying a single migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationOutcome {
    Applied,
    AlreadyValid,
    Failed,
}

/// Result of applying a single migration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationApplyResult {
    pub profile_name: String,
    pub field: ProtonPathField,
    pub old_path: String,
    pub new_path: String,
    pub outcome: MigrationOutcome,
    pub error: Option<String>,
}

/// Request to apply a single migration (received from frontend, deserialize only).
#[derive(Debug, Clone, Deserialize)]
pub struct ApplyMigrationRequest {
    pub profile_name: String,
    pub field: ProtonPathField,
    pub new_path: String,
}

/// Request to apply multiple migrations at once (deserialize only).
#[derive(Debug, Clone, Deserialize)]
pub struct BatchMigrationRequest {
    pub migrations: Vec<ApplyMigrationRequest>,
}

/// Result of a batch migration operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchMigrationResult {
    pub results: Vec<MigrationApplyResult>,
    pub applied_count: usize,
    pub failed_count: usize,
    pub skipped_count: usize,
}
