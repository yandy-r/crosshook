use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationSeverity {
    Fatal,
    Warning,
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchValidationIssue {
    pub message: String,
    pub help: String,
    pub severity: ValidationSeverity,
    /// Machine-readable issue kind for clients (e.g. `trainer_hash_mismatch`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trainer_hash_stored: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trainer_hash_current: Option<String>,
    /// Community manifest expected digest when `code` is `trainer_hash_community_mismatch`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trainer_sha256_community: Option<String>,
}

impl LaunchValidationIssue {
    pub fn trainer_hash_mismatch(stored: &str, current: &str) -> Self {
        Self {
            message: "Trainer executable SHA-256 does not match the stored baseline.".to_string(),
            help: "If you trust this trainer update, use “Update stored hash” after verifying the source. Otherwise replace the file with a known-good build.".to_string(),
            severity: ValidationSeverity::Warning,
            code: Some("trainer_hash_mismatch".to_string()),
            trainer_hash_stored: Some(stored.to_string()),
            trainer_hash_current: Some(current.to_string()),
            trainer_sha256_community: None,
        }
    }

    pub fn trainer_hash_community_advisory(expected: &str, current: &str) -> Self {
        Self {
            message: "Trainer SHA-256 differs from the community profile digest (advisory).".to_string(),
            help: "The community manifest lists a known-good hash that does not match your file. This can mean a different trainer build or a modified binary. Launch is not blocked.".to_string(),
            severity: ValidationSeverity::Warning,
            code: Some("trainer_hash_community_mismatch".to_string()),
            trainer_hash_stored: None,
            trainer_hash_current: Some(current.to_string()),
            trainer_sha256_community: Some(expected.to_string()),
        }
    }
}
