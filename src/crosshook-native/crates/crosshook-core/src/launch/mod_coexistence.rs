//! Non-blocking mod-coexistence advisory collection for preview and launch.

use std::fs;
use std::path::{Path, PathBuf};

use crate::launch::request::{LaunchRequest, LaunchValidationIssue, ValidationSeverity};
use crate::metadata::{MetadataStore, MetadataStoreError};
use crate::mods::{analyze_mod_coexistence, CoexistenceContext, ProfileModRecord};
use crate::profile::GameProfile;

pub const MAX_COEXISTENCE_ADVISORY_RECORDS: usize = 16;

/// Request-shaped collector (game/trainer launch + preview). Non-blocking:
/// returns `[]` when the DB is unavailable, the profile is unknown, there are
/// no enabled mods, or any store error occurs. Runs the pure rules only — the
/// filesystem path-missing pass is panel-only (see
/// [`analyze_profile_mod_coexistence`]) so per-keystroke previews never stat.
pub fn collect_mod_coexistence_launch_warnings(
    metadata: &MetadataStore,
    profile_id: &str,
    profile: &GameProfile,
    request: &LaunchRequest,
) -> Vec<LaunchValidationIssue> {
    if !metadata.is_available() {
        return Vec::new();
    }
    let mods = match metadata.list_enabled_profile_mods(profile_id) {
        Ok(mods) => mods,
        Err(_) => return Vec::new(),
    };
    if mods.is_empty() {
        return Vec::new();
    }

    let effective = profile.effective_profile();
    let ctx = context_from_request(&effective, request);
    advisory_issues(&mods, &ctx)
}

/// Editor-side collector (no `LaunchRequest`). Same rules plus the
/// stale-registration path check; context and game dir come from the effective
/// profile so panel and preview cannot drift.
/// Unlike the launch collector this PROPAGATES store errors so the IPC layer
/// can distinguish `metadata_unavailable` from a real failure.
pub fn analyze_profile_mod_coexistence(
    metadata: &MetadataStore,
    profile_id: &str,
    profile: &GameProfile,
) -> Result<Vec<LaunchValidationIssue>, MetadataStoreError> {
    let mods = metadata.list_enabled_profile_mods(profile_id)?;
    if mods.is_empty() {
        return Ok(Vec::new());
    }

    let effective = profile.effective_profile();
    let ctx = context_from_profile(&effective);
    let game_dir = executable_parent(&effective.game.executable_path);
    let mut issues = advisory_issues(&mods, &ctx);
    issues.extend(registered_path_missing_issues(&mods, game_dir.as_deref()));
    Ok(issues)
}

fn advisory_issues(
    mods: &[ProfileModRecord],
    ctx: &CoexistenceContext,
) -> Vec<LaunchValidationIssue> {
    analyze_mod_coexistence(mods, ctx)
        .iter()
        .map(LaunchValidationIssue::mod_coexistence_advisory)
        .collect()
}

fn context_from_request(effective: &GameProfile, request: &LaunchRequest) -> CoexistenceContext {
    CoexistenceContext {
        trainer_configured: !request.trainer_path.trim().is_empty()
            || !effective.trainer.path.trim().is_empty(),
        trainer_loading_mode: request.trainer_loading_mode,
        injection_method: effective.injection.method,
        injection_dll_names: dll_basenames(&effective.injection.dll_paths),
        mangohud_enabled: request.mangohud.enabled,
        gamescope_enabled: request.gamescope.enabled,
    }
}

fn context_from_profile(effective: &GameProfile) -> CoexistenceContext {
    CoexistenceContext {
        trainer_configured: !effective.trainer.path.trim().is_empty(),
        trainer_loading_mode: effective.trainer.loading_mode,
        injection_method: effective.injection.method,
        injection_dll_names: dll_basenames(&effective.injection.dll_paths),
        mangohud_enabled: effective.launch.mangohud.enabled,
        gamescope_enabled: effective.launch.gamescope.enabled,
    }
}

fn dll_basenames(dll_paths: &[String]) -> Vec<String> {
    dll_paths
        .iter()
        .filter_map(|path| Path::new(path).file_name().and_then(|name| name.to_str()))
        .map(str::to_ascii_lowercase)
        .collect()
}

fn executable_parent(executable_path: &str) -> Option<PathBuf> {
    let trimmed = executable_path.trim();
    if trimmed.is_empty() {
        return None;
    }
    Path::new(trimmed).parent().map(Path::to_path_buf)
}

/// One Info issue per enabled mod with at least one registered path that no
/// longer exists. Relative paths resolve against `game_dir` (skipped when it
/// is unknown); absolute paths are always checked. Bounded — ≤ 32 paths per
/// mod is enforced at the store boundary — and symlink-safe.
fn registered_path_missing_issues(
    mods: &[ProfileModRecord],
    game_dir: Option<&Path>,
) -> Vec<LaunchValidationIssue> {
    let mut issues = Vec::new();
    for record in mods.iter().filter(|record| record.enabled) {
        let first_missing = record.paths.iter().find_map(|raw| {
            let path = Path::new(raw);
            let resolved = if path.is_absolute() {
                path.to_path_buf()
            } else {
                game_dir?.join(path)
            };
            fs::symlink_metadata(&resolved).is_err().then_some(raw)
        });
        if let Some(missing) = first_missing {
            issues.push(registered_path_missing_issue(&record.name, missing));
        }
    }
    issues
}

fn registered_path_missing_issue(mod_name: &str, missing_path: &str) -> LaunchValidationIssue {
    LaunchValidationIssue {
        message: format!(
            "Registered path for mod “{mod_name}” no longer exists: {missing_path}"
        ),
        help: "The registration is stale. Update or remove this mod entry; advisories may be inaccurate until then.".to_string(),
        severity: ValidationSeverity::Info,
        code: Some("mod_coexistence_registered_path_missing".to_string()),
        trainer_hash_stored: None,
        trainer_hash_current: None,
        trainer_sha256_community: None,
        hook_id: None,
        hook_name: None,
        hook_stage: None,
        hook_exit_code: None,
        hook_timed_out: None,
    }
}
