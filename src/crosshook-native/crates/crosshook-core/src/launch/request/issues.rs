use serde::{Deserialize, Serialize};

use crate::profile::HookStage;

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hook_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hook_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hook_stage: Option<HookStage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hook_exit_code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hook_timed_out: Option<bool>,
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
            hook_id: None,
            hook_name: None,
            hook_stage: None,
            hook_exit_code: None,
            hook_timed_out: None,
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
            hook_id: None,
            hook_name: None,
            hook_stage: None,
            hook_exit_code: None,
            hook_timed_out: None,
        }
    }

    pub fn launch_hook_skipped(
        hook: &crate::profile::LaunchHook,
        message: &str,
        help: &str,
        code: Option<&str>,
    ) -> Self {
        Self {
            message: message.to_string(),
            help: help.to_string(),
            severity: ValidationSeverity::Warning,
            code: code.map(str::to_string),
            trainer_hash_stored: None,
            trainer_hash_current: None,
            trainer_sha256_community: None,
            hook_id: Some(hook.id.clone()),
            hook_name: Some(hook.name.clone()),
            hook_stage: Some(hook.stage),
            hook_exit_code: None,
            hook_timed_out: Some(false),
        }
    }

    pub fn launch_hook_timed_out(hook: &crate::profile::LaunchHook) -> Self {
        Self {
            message: "Launch hook timed out and was skipped.".to_string(),
            help: "Launch continues by default. Shorten the hook script or disable the hook if it cannot finish in time.".to_string(),
            severity: ValidationSeverity::Warning,
            code: Some("launch_hook_timed_out".to_string()),
            trainer_hash_stored: None,
            trainer_hash_current: None,
            trainer_sha256_community: None,
            hook_id: Some(hook.id.clone()),
            hook_name: Some(hook.name.clone()),
            hook_stage: Some(hook.stage),
            hook_exit_code: None,
            hook_timed_out: Some(true),
        }
    }

    pub fn launch_hook_non_zero_exit(hook: &crate::profile::LaunchHook, exit_code: i32) -> Self {
        Self {
            message: "Launch hook exited with a non-zero status.".to_string(),
            help: "Launch continues by default. Check the hook path, permissions, or script output before relying on this hook.".to_string(),
            severity: ValidationSeverity::Warning,
            code: Some("launch_hook_non_zero_exit".to_string()),
            trainer_hash_stored: None,
            trainer_hash_current: None,
            trainer_sha256_community: None,
            hook_id: Some(hook.id.clone()),
            hook_name: Some(hook.name.clone()),
            hook_stage: Some(hook.stage),
            hook_exit_code: Some(exit_code),
            hook_timed_out: Some(false),
        }
    }
}
