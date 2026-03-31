use std::fs;
use std::io::ErrorKind;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::launch::request::{METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH};
use crate::profile::models::{resolve_launch_method, GameProfile};
use crate::profile::toml_store::ProfileStore;

/// Profile-level health roll-up.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Healthy,
    Stale,
    Broken,
}

/// Per-issue severity — distinct from `ValidationSeverity` which always returns Fatal.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthIssueSeverity {
    Error,
    Warning,
    Info,
}

/// A single path-field issue found during health check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthIssue {
    pub field: String,
    pub path: String,
    pub message: String,
    pub remediation: String,
    pub severity: HealthIssueSeverity,
}

/// Per-profile health check result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileHealthReport {
    pub name: String,
    pub status: HealthStatus,
    pub launch_method: String,
    pub issues: Vec<HealthIssue>,
    pub checked_at: String,
}

/// Batch health check result across all profiles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckSummary {
    pub profiles: Vec<ProfileHealthReport>,
    pub healthy_count: usize,
    pub stale_count: usize,
    pub broken_count: usize,
    pub total_count: usize,
    pub validated_at: String,
}

/// Classify a path check result (missing file vs. wrong type / inaccessible) into a `HealthIssue`.
///
/// Returns `None` when the path is healthy (present, correct type, accessible).
fn check_file_path(
    field: &str,
    path: &str,
    severity_on_broken: HealthIssueSeverity,
) -> Option<(HealthIssue, bool /* is_stale */)> {
    if path.trim().is_empty() {
        return None;
    }

    match fs::metadata(path) {
        Ok(meta) if meta.is_file() => {
            // Exists and is a file — healthy
            None
        }
        Ok(_) => {
            // Exists but wrong type (directory, symlink to dir, etc.)
            Some((
                HealthIssue {
                    field: field.to_string(),
                    path: path.to_string(),
                    message: format!("Path exists but is not a file: {path}"),
                    remediation: "Select the file itself, not a directory or other path type."
                        .to_string(),
                    severity: severity_on_broken,
                },
                false,
            ))
        }
        Err(err) if err.kind() == ErrorKind::NotFound => {
            // Missing from disk → Stale
            Some((
                HealthIssue {
                    field: field.to_string(),
                    path: path.to_string(),
                    message: format!("Path does not exist: {path}"),
                    remediation: "Re-browse to the file or verify the path is correct.".to_string(),
                    severity: HealthIssueSeverity::Warning,
                },
                true,
            ))
        }
        Err(err) if err.kind() == ErrorKind::PermissionDenied => Some((
            HealthIssue {
                field: field.to_string(),
                path: path.to_string(),
                message: format!("Path is not accessible (permission denied): {path}"),
                remediation: "Check file permissions (e.g. chmod a+r).".to_string(),
                severity: severity_on_broken,
            },
            false,
        )),
        Err(err) => {
            // Other I/O errors are treated as broken
            Some((
                HealthIssue {
                    field: field.to_string(),
                    path: path.to_string(),
                    message: format!("Could not access path: {err}"),
                    remediation: "Verify the path is valid and accessible.".to_string(),
                    severity: severity_on_broken,
                },
                false,
            ))
        }
    }
}

/// Check a required file field. Empty path → `Broken`. Missing → `Stale`. Wrong type → `Broken`.
fn check_required_file(field: &str, path: &str) -> Option<(HealthIssue, bool)> {
    if path.trim().is_empty() {
        return Some((
            HealthIssue {
                field: field.to_string(),
                path: String::new(),
                message: format!("Required field '{field}' is not configured."),
                remediation: format!("Browse to or enter a value for '{field}'."),
                severity: HealthIssueSeverity::Error,
            },
            false,
        ));
    }
    check_file_path(field, path, HealthIssueSeverity::Error)
}

/// Check a required directory field. Empty → `Broken`. Missing → `Stale`. Wrong type → `Broken`.
fn check_required_directory(field: &str, path: &str) -> Option<(HealthIssue, bool)> {
    if path.trim().is_empty() {
        return Some((
            HealthIssue {
                field: field.to_string(),
                path: String::new(),
                message: format!("Required field '{field}' is not configured."),
                remediation: format!("Browse to or enter a value for '{field}'."),
                severity: HealthIssueSeverity::Error,
            },
            false,
        ));
    }

    match fs::metadata(path) {
        Ok(meta) if meta.is_dir() => None,
        Ok(_) => Some((
            HealthIssue {
                field: field.to_string(),
                path: path.to_string(),
                message: format!("Path exists but is not a directory: {path}"),
                remediation: "Select the directory itself, not a file inside it.".to_string(),
                severity: HealthIssueSeverity::Error,
            },
            false,
        )),
        Err(err) if err.kind() == ErrorKind::NotFound => Some((
            HealthIssue {
                field: field.to_string(),
                path: path.to_string(),
                message: format!("Directory does not exist: {path}"),
                remediation: "Re-browse to the directory or verify the path is correct."
                    .to_string(),
                severity: HealthIssueSeverity::Warning,
            },
            true,
        )),
        Err(err) if err.kind() == ErrorKind::PermissionDenied => Some((
            HealthIssue {
                field: field.to_string(),
                path: path.to_string(),
                message: format!("Directory is not accessible (permission denied): {path}"),
                remediation: "Check directory permissions (e.g. chmod a+rx).".to_string(),
                severity: HealthIssueSeverity::Error,
            },
            false,
        )),
        Err(err) => Some((
            HealthIssue {
                field: field.to_string(),
                path: path.to_string(),
                message: format!("Could not access directory: {err}"),
                remediation: "Verify the path is valid and accessible.".to_string(),
                severity: HealthIssueSeverity::Error,
            },
            false,
        )),
    }
}

/// Check a required executable file field. Empty → `Broken`. Missing → `Stale`. Not executable → `Broken`.
fn check_required_executable(field: &str, path: &str) -> Option<(HealthIssue, bool)> {
    if path.trim().is_empty() {
        return Some((
            HealthIssue {
                field: field.to_string(),
                path: String::new(),
                message: format!("Required field '{field}' is not configured."),
                remediation: format!("Browse to or enter a value for '{field}'."),
                severity: HealthIssueSeverity::Error,
            },
            false,
        ));
    }

    match fs::metadata(path) {
        Ok(meta) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if !meta.is_file() {
                    return Some((
                        HealthIssue {
                            field: field.to_string(),
                            path: path.to_string(),
                            message: format!("Path exists but is not a file: {path}"),
                            remediation: "Select the executable file itself.".to_string(),
                            severity: HealthIssueSeverity::Error,
                        },
                        false,
                    ));
                }
                if meta.permissions().mode() & 0o111 == 0 {
                    return Some((
                        HealthIssue {
                            field: field.to_string(),
                            path: path.to_string(),
                            message: format!(
                                "File is not executable (no execute permission): {path}"
                            ),
                            remediation: "Run 'chmod +x' on the file to make it executable."
                                .to_string(),
                            severity: HealthIssueSeverity::Error,
                        },
                        false,
                    ));
                }
                None
            }
            #[cfg(not(unix))]
            {
                if !meta.is_file() {
                    return Some((
                        HealthIssue {
                            field: field.to_string(),
                            path: path.to_string(),
                            message: format!("Path exists but is not a file: {path}"),
                            remediation: "Select the executable file itself.".to_string(),
                            severity: HealthIssueSeverity::Error,
                        },
                        false,
                    ));
                }
                None
            }
        }
        Err(err) if err.kind() == ErrorKind::NotFound => Some((
            HealthIssue {
                field: field.to_string(),
                path: path.to_string(),
                message: format!("Executable does not exist: {path}"),
                remediation: "Re-browse to the executable or verify the path is correct."
                    .to_string(),
                severity: HealthIssueSeverity::Warning,
            },
            true,
        )),
        Err(err) if err.kind() == ErrorKind::PermissionDenied => Some((
            HealthIssue {
                field: field.to_string(),
                path: path.to_string(),
                message: format!("Executable is not accessible (permission denied): {path}"),
                remediation: "Check file permissions (e.g. chmod a+rx).".to_string(),
                severity: HealthIssueSeverity::Error,
            },
            false,
        )),
        Err(err) => Some((
            HealthIssue {
                field: field.to_string(),
                path: path.to_string(),
                message: format!("Could not access executable: {err}"),
                remediation: "Verify the path is valid and accessible.".to_string(),
                severity: HealthIssueSeverity::Error,
            },
            false,
        )),
    }
}

/// Check an optional path field. Empty → no issue. Missing or inaccessible → `Info`.
fn check_optional_path(field: &str, path: &str) -> Option<HealthIssue> {
    if path.trim().is_empty() {
        return None;
    }

    match fs::metadata(path) {
        Ok(_) => None,
        Err(err) if err.kind() == ErrorKind::NotFound => Some(HealthIssue {
            field: field.to_string(),
            path: path.to_string(),
            message: format!("Optional path does not exist: {path}"),
            remediation: format!("Browse to or clear the '{field}' field if no longer needed."),
            severity: HealthIssueSeverity::Info,
        }),
        Err(_) => Some(HealthIssue {
            field: field.to_string(),
            path: path.to_string(),
            message: format!("Optional path is not accessible: {path}"),
            remediation: format!("Verify the '{field}' path or clear it if no longer needed."),
            severity: HealthIssueSeverity::Info,
        }),
    }
}

/// Validates path fields of a `GameProfile` and returns a `ProfileHealthReport`.
///
/// Method-aware: only fields required by the resolved launch method are checked as required.
/// All populated optional fields (icon_path, working_directory) are validated at Info severity.
pub fn check_profile_health(name: &str, profile: &GameProfile) -> ProfileHealthReport {
    let effective_profile = profile.effective_profile();
    let launch_method = resolve_launch_method(&effective_profile).to_string();
    let mut issues: Vec<HealthIssue> = Vec::new();
    let mut has_stale = false;
    let mut has_broken = false;

    // Collect required-field results — each returns (issue, is_stale)
    let mut required_results: Vec<Option<(HealthIssue, bool)>> = Vec::new();

    // game.executable_path — required for all methods
    required_results.push(check_required_file(
        "game.executable_path",
        &effective_profile.game.executable_path,
    ));

    // trainer.path — required only if non-empty
    if !effective_profile.trainer.path.trim().is_empty() {
        required_results.push(check_file_path(
            "trainer.path",
            &effective_profile.trainer.path,
            HealthIssueSeverity::Error,
        ));
    }

    // injection.dll_paths — each non-empty entry must exist as a file
    for (i, dll_path) in effective_profile.injection.dll_paths.iter().enumerate() {
        if !dll_path.trim().is_empty() {
            required_results.push(check_file_path(
                &format!("injection.dll_paths[{i}]"),
                dll_path,
                HealthIssueSeverity::Error,
            ));
        }
    }

    // Method-specific required fields
    match launch_method.as_str() {
        METHOD_STEAM_APPLAUNCH => {
            required_results.push(check_required_directory(
                "steam.compatdata_path",
                &effective_profile.steam.compatdata_path,
            ));
            required_results.push(check_required_executable(
                "steam.proton_path",
                &effective_profile.steam.proton_path,
            ));
        }
        METHOD_PROTON_RUN => {
            required_results.push(check_required_directory(
                "runtime.prefix_path",
                &effective_profile.runtime.prefix_path,
            ));
            required_results.push(check_required_executable(
                "runtime.proton_path",
                &effective_profile.runtime.proton_path,
            ));
        }
        _ => {
            // native — no additional required path fields
        }
    }

    // Process required-field results
    for result in required_results {
        if let Some((issue, stale)) = result {
            if stale {
                has_stale = true;
            } else {
                has_broken = true;
            }
            issues.push(issue);
        }
    }

    // Optional fields — checked at Info severity regardless of method
    if let Some(issue) = check_optional_path(
        "steam.launcher.icon_path",
        &effective_profile.steam.launcher.icon_path,
    ) {
        issues.push(issue);
    }
    if let Some(issue) = check_optional_path(
        "runtime.working_directory",
        &effective_profile.runtime.working_directory,
    ) {
        issues.push(issue);
    }

    // Determine overall status.
    // Unconfigured profiles (all empty required fields) also classify as Broken per business rules;
    // the UI presents them with badge-only (no banner) based on all issues having empty path fields.
    let status = if has_broken {
        HealthStatus::Broken
    } else if has_stale {
        HealthStatus::Stale
    } else {
        HealthStatus::Healthy
    };

    ProfileHealthReport {
        name: name.to_string(),
        status,
        launch_method,
        issues,
        checked_at: Utc::now().to_rfc3339(),
    }
}

/// Like [`batch_check_health`], but invokes `enrich` after each successful `check_profile_health`
/// so callers can attach SQLite-backed checks (e.g. offline readiness) using one `Connection`.
pub fn batch_check_health_with_enrich<F>(store: &ProfileStore, mut enrich: F) -> HealthCheckSummary
where
    F: FnMut(&str, &GameProfile, &mut ProfileHealthReport),
{
    let now = Utc::now().to_rfc3339();

    let names = match store.list() {
        Ok(names) => names,
        Err(err) => {
            return HealthCheckSummary {
                profiles: vec![ProfileHealthReport {
                    name: "<unknown>".to_string(),
                    status: HealthStatus::Broken,
                    launch_method: String::new(),
                    issues: vec![HealthIssue {
                        field: String::new(),
                        path: String::new(),
                        message: format!("Could not list profiles: {err}"),
                        remediation: "Check filesystem permissions for the profiles directory."
                            .to_string(),
                        severity: HealthIssueSeverity::Error,
                    }],
                    checked_at: now.clone(),
                }],
                healthy_count: 0,
                stale_count: 0,
                broken_count: 1,
                total_count: 1,
                validated_at: now,
            };
        }
    };

    let mut profiles: Vec<ProfileHealthReport> = Vec::with_capacity(names.len());

    for name in &names {
        let report = match store.load(name) {
            Ok(profile) => {
                let mut report = check_profile_health(name, &profile);
                enrich(name, &profile, &mut report);
                report
            }
            Err(err) => ProfileHealthReport {
                name: name.clone(),
                status: HealthStatus::Broken,
                launch_method: String::new(),
                issues: vec![HealthIssue {
                    field: String::new(),
                    path: String::new(),
                    message: format!("Profile could not be loaded: {err}"),
                    remediation: "The profile TOML may be malformed. Delete and re-create the profile, or edit the file manually.".to_string(),
                    severity: HealthIssueSeverity::Error,
                }],
                checked_at: Utc::now().to_rfc3339(),
            },
        };
        profiles.push(report);
    }

    let healthy_count = profiles
        .iter()
        .filter(|r| matches!(r.status, HealthStatus::Healthy))
        .count();
    let stale_count = profiles
        .iter()
        .filter(|r| matches!(r.status, HealthStatus::Stale))
        .count();
    let broken_count = profiles
        .iter()
        .filter(|r| matches!(r.status, HealthStatus::Broken))
        .count();
    let total_count = profiles.len();

    HealthCheckSummary {
        profiles,
        healthy_count,
        stale_count,
        broken_count,
        total_count,
        validated_at: now,
    }
}

/// Runs `check_profile_health` for every profile in the store and returns a summary.
///
/// Errors loading individual profiles are captured as `Broken` entries and do not abort
/// the batch — this function never propagates `ProfileStoreError` from the per-profile loop.
pub fn batch_check_health(store: &ProfileStore) -> HealthCheckSummary {
    let now = Utc::now().to_rfc3339();

    let names = match store.list() {
        Ok(names) => names,
        Err(err) => {
            // Cannot enumerate profiles at all — return an empty summary with one sentinel
            return HealthCheckSummary {
                profiles: vec![ProfileHealthReport {
                    name: "<unknown>".to_string(),
                    status: HealthStatus::Broken,
                    launch_method: String::new(),
                    issues: vec![HealthIssue {
                        field: String::new(),
                        path: String::new(),
                        message: format!("Could not list profiles: {err}"),
                        remediation: "Check filesystem permissions for the profiles directory."
                            .to_string(),
                        severity: HealthIssueSeverity::Error,
                    }],
                    checked_at: now.clone(),
                }],
                healthy_count: 0,
                stale_count: 0,
                broken_count: 1,
                total_count: 1,
                validated_at: now,
            };
        }
    };

    let mut profiles: Vec<ProfileHealthReport> = Vec::with_capacity(names.len());

    for name in &names {
        let report = match store.load(name) {
            Ok(profile) => check_profile_health(name, &profile),
            Err(err) => ProfileHealthReport {
                name: name.clone(),
                status: HealthStatus::Broken,
                launch_method: String::new(),
                issues: vec![HealthIssue {
                    field: String::new(),
                    path: String::new(),
                    message: format!("Profile could not be loaded: {err}"),
                    remediation: "The profile TOML may be malformed. Delete and re-create the profile, or edit the file manually.".to_string(),
                    severity: HealthIssueSeverity::Error,
                }],
                checked_at: Utc::now().to_rfc3339(),
            },
        };
        profiles.push(report);
    }

    let healthy_count = profiles
        .iter()
        .filter(|r| matches!(r.status, HealthStatus::Healthy))
        .count();
    let stale_count = profiles
        .iter()
        .filter(|r| matches!(r.status, HealthStatus::Stale))
        .count();
    let broken_count = profiles
        .iter()
        .filter(|r| matches!(r.status, HealthStatus::Broken))
        .count();
    let total_count = profiles.len();

    HealthCheckSummary {
        profiles,
        healthy_count,
        stale_count,
        broken_count,
        total_count,
        validated_at: now,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use tempfile::tempdir;

    use crate::profile::{
        GameProfile, GameSection, InjectionSection, LaunchSection, LauncherSection, RuntimeSection,
        SteamSection, TrainerSection,
    };

    /// Create a real executable file at `path`.
    fn make_executable(path: &Path) {
        fs::write(path, b"#!/bin/sh\n").expect("write executable");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut perms = fs::metadata(path).expect("metadata").permissions();
            perms.set_mode(0o755);
            fs::set_permissions(path, perms).expect("chmod");
        }
    }

    /// Build a `GameProfile` for `steam_applaunch` where all configured paths exist.
    fn healthy_steam_profile(tmp: &Path) -> GameProfile {
        let game_exe = tmp.join("game.exe");
        let trainer = tmp.join("trainer.exe");
        let dll = tmp.join("mod.dll");
        let compatdata = tmp.join("compatdata");
        let proton = tmp.join("proton");

        make_executable(&game_exe);
        make_executable(&trainer);
        fs::write(&dll, b"MZ").expect("write dll");
        fs::create_dir_all(&compatdata).expect("mkdir compatdata");
        make_executable(&proton);

        GameProfile {
            game: GameSection {
                name: "Test Game".to_string(),
                executable_path: game_exe.to_string_lossy().to_string(),
            },
            trainer: TrainerSection {
                path: trainer.to_string_lossy().to_string(),
                kind: "fling".to_string(),
                loading_mode: crate::profile::TrainerLoadingMode::SourceDirectory,
                trainer_type: "unknown".to_string(),
            },
            injection: InjectionSection {
                dll_paths: vec![dll.to_string_lossy().to_string()],
                inject_on_launch: vec![true],
            },
            steam: SteamSection {
                enabled: true,
                app_id: "12345".to_string(),
                compatdata_path: compatdata.to_string_lossy().to_string(),
                proton_path: proton.to_string_lossy().to_string(),
                launcher: LauncherSection {
                    icon_path: String::new(),
                    display_name: String::new(),
                },
            },
            runtime: RuntimeSection::default(),
            launch: LaunchSection {
                method: "steam_applaunch".to_string(),
                ..Default::default()
            },
            local_override: crate::profile::LocalOverrideSection::default(),
        }
    }

    #[test]
    fn healthy_profile_reports_healthy_status() {
        let tmp = tempdir().expect("tempdir");
        let profile = healthy_steam_profile(tmp.path());
        let report = check_profile_health("test-game", &profile);

        assert!(
            matches!(report.status, HealthStatus::Healthy),
            "expected Healthy, got {:?}; issues: {:?}",
            report.status,
            report.issues
        );
        assert!(report.issues.is_empty());
        assert_eq!(report.name, "test-game");
        assert_eq!(report.launch_method, "steam_applaunch");
    }

    #[test]
    fn missing_game_exe_reports_stale() {
        let tmp = tempdir().expect("tempdir");
        let mut profile = healthy_steam_profile(tmp.path());
        profile.game.executable_path = tmp
            .path()
            .join("nonexistent.exe")
            .to_string_lossy()
            .to_string();

        let report = check_profile_health("stale-game", &profile);

        assert!(
            matches!(report.status, HealthStatus::Stale),
            "expected Stale, got {:?}",
            report.status
        );
        assert!(report
            .issues
            .iter()
            .any(|i| i.field == "game.executable_path"));
    }

    #[test]
    fn game_exe_is_directory_reports_broken() {
        let tmp = tempdir().expect("tempdir");
        let dir_path = tmp.path().join("itsadir");
        fs::create_dir_all(&dir_path).expect("mkdir");

        let mut profile = healthy_steam_profile(tmp.path());
        profile.game.executable_path = dir_path.to_string_lossy().to_string();

        let report = check_profile_health("broken-game", &profile);

        assert!(
            matches!(report.status, HealthStatus::Broken),
            "expected Broken, got {:?}",
            report.status
        );
        assert!(report
            .issues
            .iter()
            .any(|i| i.field == "game.executable_path"));
    }

    #[test]
    fn unconfigured_profile_reports_broken() {
        let profile = GameProfile::default();
        let report = check_profile_health("empty-profile", &profile);

        // game.executable_path is required for all methods — empty → Broken
        assert!(
            matches!(report.status, HealthStatus::Broken),
            "expected Broken for empty profile, got {:?}",
            report.status
        );
        assert!(report
            .issues
            .iter()
            .any(|i| i.field == "game.executable_path"));
    }

    #[test]
    fn missing_proton_reports_stale_for_steam_applaunch() {
        let tmp = tempdir().expect("tempdir");
        let mut profile = healthy_steam_profile(tmp.path());
        // Point proton_path at a nonexistent path
        profile.steam.proton_path = tmp.path().join("gone_proton").to_string_lossy().to_string();

        let report = check_profile_health("stale-steam", &profile);

        assert!(
            matches!(report.status, HealthStatus::Stale),
            "expected Stale (missing proton), got {:?}",
            report.status
        );
        assert!(report.issues.iter().any(|i| i.field == "steam.proton_path"));
    }

    #[cfg(unix)]
    #[test]
    fn proton_path_not_executable_reports_broken() {
        let tmp = tempdir().expect("tempdir");
        let mut profile = healthy_steam_profile(tmp.path());

        // Create a non-executable file as proton
        let non_exec = tmp.path().join("proton_no_exec");
        fs::write(&non_exec, b"data").expect("write");
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&non_exec).expect("meta").permissions();
        perms.set_mode(0o644);
        fs::set_permissions(&non_exec, perms).expect("chmod");
        profile.steam.proton_path = non_exec.to_string_lossy().to_string();

        let report = check_profile_health("broken-proton", &profile);

        assert!(
            matches!(report.status, HealthStatus::Broken),
            "expected Broken (non-executable proton), got {:?}",
            report.status
        );
        assert!(report.issues.iter().any(|i| i.field == "steam.proton_path"));
    }

    #[test]
    fn batch_check_health_returns_all_profiles() {
        let tmp = tempdir().expect("tempdir");
        let store = ProfileStore::with_base_path(tmp.path().join("profiles"));

        let profile = healthy_steam_profile(tmp.path());
        store.save("game-a", &profile).expect("save game-a");
        store.save("game-b", &profile).expect("save game-b");

        let summary = batch_check_health(&store);

        assert_eq!(summary.total_count, 2);
        assert_eq!(summary.profiles.len(), 2);
    }

    #[test]
    fn batch_check_health_isolates_toml_parse_error() {
        let tmp = tempdir().expect("tempdir");
        let store = ProfileStore::with_base_path(tmp.path().join("profiles"));

        // Save a valid profile
        let profile = healthy_steam_profile(tmp.path());
        store.save("valid-profile", &profile).expect("save valid");

        // Write an intentionally malformed TOML file
        let bad_path = store.base_path.join("broken-toml.toml");
        fs::create_dir_all(&store.base_path).expect("mkdir profiles");
        fs::write(&bad_path, b"[invalid toml content %%% @@").expect("write bad toml");

        let summary = batch_check_health(&store);

        // Should have 2 profiles total: 1 valid, 1 broken
        assert_eq!(summary.total_count, 2);
        assert_eq!(summary.broken_count, 1);

        let broken = summary
            .profiles
            .iter()
            .find(|r| r.name == "broken-toml")
            .expect("broken-toml report missing");
        assert!(matches!(broken.status, HealthStatus::Broken));
        assert!(!broken.issues.is_empty());
    }

    #[test]
    fn batch_check_health_empty_store_returns_empty_summary() {
        let tmp = tempdir().expect("tempdir");
        let store = ProfileStore::with_base_path(tmp.path().join("profiles"));

        let summary = batch_check_health(&store);

        assert_eq!(summary.total_count, 0);
        assert_eq!(summary.healthy_count, 0);
        assert_eq!(summary.stale_count, 0);
        assert_eq!(summary.broken_count, 0);
    }

    #[test]
    fn proton_run_method_checks_runtime_prefix_not_steam() {
        let tmp = tempdir().expect("tempdir");

        let game_exe = tmp.path().join("game.exe");
        make_executable(&game_exe);

        let prefix = tmp.path().join("pfx");
        fs::create_dir_all(&prefix).expect("mkdir prefix");

        let proton = tmp.path().join("proton");
        make_executable(&proton);

        let profile = GameProfile {
            game: GameSection {
                name: "Proton Game".to_string(),
                executable_path: game_exe.to_string_lossy().to_string(),
            },
            trainer: TrainerSection::default(),
            injection: InjectionSection::default(),
            steam: SteamSection::default(),
            runtime: RuntimeSection {
                prefix_path: prefix.to_string_lossy().to_string(),
                proton_path: proton.to_string_lossy().to_string(),
                working_directory: String::new(),
            },
            launch: LaunchSection {
                method: "proton_run".to_string(),
                ..Default::default()
            },
            local_override: crate::profile::LocalOverrideSection::default(),
        };

        let report = check_profile_health("proton-run-game", &profile);

        assert!(
            matches!(report.status, HealthStatus::Healthy),
            "expected Healthy for proton_run profile with all paths present, got {:?}; issues: {:?}",
            report.status,
            report.issues
        );
    }
}
