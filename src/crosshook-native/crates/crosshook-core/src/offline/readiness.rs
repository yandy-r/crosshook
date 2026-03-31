use std::path::Path;

use chrono::Utc;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::launch::request::{METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH};
use crate::metadata::offline_store;
use crate::metadata::MetadataStoreError;
use crate::profile::health::{HealthIssue, HealthIssueSeverity, ProfileHealthReport};
use crate::profile::{resolve_launch_method, GameProfile};

use super::hash::verify_and_cache_trainer_hash;
use super::trainer_type::global_trainer_type_catalog;

/// Integer flags for `offline_readiness_snapshots` (not serialized over IPC).
#[derive(Debug, Clone, Default)]
pub struct OfflineReadinessPersistHints {
    pub trainer_present: i64,
    pub trainer_hash_valid: i64,
    pub trainer_activated: i64,
    pub proton_available: i64,
    pub community_tap_cached: i64,
    pub network_required: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineReadinessReport {
    pub profile_name: String,
    pub score: u8,
    pub readiness_state: String,
    pub trainer_type: String,
    pub checks: Vec<HealthIssue>,
    pub blocking_reasons: Vec<String>,
    pub checked_at: String,
    #[serde(skip)]
    pub persist: OfflineReadinessPersistHints,
}

/// Pure readiness scoring (no I/O).
pub fn compute_offline_readiness(
    profile_name: &str,
    trainer_type: &str,
    trainer_present: bool,
    trainer_hash_valid: bool,
    game_present: bool,
    proton_available: bool,
    prefix_exists: bool,
    network_required: bool,
    score_cap: Option<u8>,
) -> OfflineReadinessReport {
    let checked_at = Utc::now().to_rfc3339();
    let mut checks: Vec<HealthIssue> = Vec::new();
    let mut blocking_reasons: Vec<String> = Vec::new();

    let mut score: u32 = 0;

    if trainer_present {
        score += 30;
        checks.push(HealthIssue {
            field: "trainer_present".to_string(),
            path: String::new(),
            message: "Trainer executable is present.".to_string(),
            remediation: String::new(),
            severity: HealthIssueSeverity::Info,
        });
    } else {
        checks.push(HealthIssue {
            field: "trainer_present".to_string(),
            path: String::new(),
            message: "No trainer executable configured or file is missing.".to_string(),
            remediation: "Set a valid trainer path in the profile.".to_string(),
            severity: HealthIssueSeverity::Warning,
        });
        blocking_reasons.push("Trainer executable missing".to_string());
    }

    if trainer_present {
        if trainer_hash_valid {
            score += 15;
            checks.push(HealthIssue {
                field: "trainer_hash_valid".to_string(),
                path: String::new(),
                message: "Trainer file hash verified.".to_string(),
                remediation: String::new(),
                severity: HealthIssueSeverity::Info,
            });
        } else {
            checks.push(HealthIssue {
                field: "trainer_hash_valid".to_string(),
                path: String::new(),
                message: "Trainer hash could not be verified (file unreadable or empty).".to_string(),
                remediation: "Ensure the trainer file is readable.".to_string(),
                severity: HealthIssueSeverity::Warning,
            });
            blocking_reasons.push("Trainer hash not verified".to_string());
        }
    } else {
        // Hash dimension is N/A without a trainer file; keep the 15-point weight (100 - 30 = 70 baseline).
        score += 15;
        checks.push(HealthIssue {
            field: "trainer_hash_valid".to_string(),
            path: String::new(),
            message: "Trainer hash skipped — no trainer file configured.".to_string(),
            remediation: String::new(),
            severity: HealthIssueSeverity::Info,
        });
    }

    if game_present {
        score += 20;
        checks.push(HealthIssue {
            field: "game_present".to_string(),
            path: String::new(),
            message: "Game executable is present.".to_string(),
            remediation: String::new(),
            severity: HealthIssueSeverity::Info,
        });
    } else {
        checks.push(HealthIssue {
            field: "game_present".to_string(),
            path: String::new(),
            message: "Game executable missing or not configured.".to_string(),
            remediation: "Set a valid game executable path.".to_string(),
            severity: HealthIssueSeverity::Warning,
        });
        blocking_reasons.push("Game executable missing".to_string());
    }

    if proton_available {
        score += 15;
        checks.push(HealthIssue {
            field: "proton_available".to_string(),
            path: String::new(),
            message: "Proton path is configured.".to_string(),
            remediation: String::new(),
            severity: HealthIssueSeverity::Info,
        });
    } else {
        checks.push(HealthIssue {
            field: "proton_available".to_string(),
            path: String::new(),
            message: "Proton not configured for this profile.".to_string(),
            remediation: "Configure Steam compatdata / Proton in the profile.".to_string(),
            severity: HealthIssueSeverity::Warning,
        });
        blocking_reasons.push("Proton not available".to_string());
    }

    if prefix_exists {
        score += 10;
        checks.push(HealthIssue {
            field: "prefix_exists".to_string(),
            path: String::new(),
            message: "Wine prefix directory exists.".to_string(),
            remediation: String::new(),
            severity: HealthIssueSeverity::Info,
        });
    } else {
        checks.push(HealthIssue {
            field: "prefix_exists".to_string(),
            path: String::new(),
            message: "Wine prefix path missing or not a directory.".to_string(),
            remediation: "Create or select a valid prefix path.".to_string(),
            severity: HealthIssueSeverity::Warning,
        });
    }

    if !network_required {
        score += 10;
        checks.push(HealthIssue {
            field: "network_not_required".to_string(),
            path: String::new(),
            message: "This trainer type does not require network for offline play.".to_string(),
            remediation: String::new(),
            severity: HealthIssueSeverity::Info,
        });
    } else {
        checks.push(HealthIssue {
            field: "network_required".to_string(),
            path: String::new(),
            message: "This trainer type may require network connectivity.".to_string(),
            remediation: "Expect limited offline readiness for this trainer vendor.".to_string(),
            severity: HealthIssueSeverity::Warning,
        });
    }

    let cap = score_cap.unwrap_or(100).min(100);
    let final_score = score.min(u32::from(cap)) as u8;

    let readiness_state = if !trainer_present {
        "unconfigured".to_string()
    } else if !blocking_reasons.is_empty() && final_score < 50 {
        "blocked".to_string()
    } else if final_score >= 80 {
        "ready".to_string()
    } else if final_score >= 50 {
        "degraded".to_string()
    } else {
        "blocked".to_string()
    };

    OfflineReadinessReport {
        profile_name: profile_name.to_string(),
        score: final_score,
        readiness_state,
        trainer_type: trainer_type.to_string(),
        checks,
        blocking_reasons,
        checked_at,
        persist: OfflineReadinessPersistHints::default(),
    }
}

/// Resolve paths on disk, consult hash cache, then score offline readiness.
pub fn check_offline_preflight(
    profile_name: &str,
    profile_id: &str,
    profile: &GameProfile,
    conn: &Connection,
) -> Result<OfflineReadinessReport, MetadataStoreError> {
    let effective = profile.effective_profile();
    let resolved_method = resolve_launch_method(&effective);
    let catalog = global_trainer_type_catalog();
    let tt_id = effective.trainer.trainer_type.as_str();
    let entry = catalog.lookup(tt_id);
    let network_required = entry.map(|e| e.requires_network).unwrap_or(false);
    let score_cap = entry.and_then(|e| e.score_cap);

    let trainer_path = effective.trainer.path.trim();
    let trainer_present = !trainer_path.is_empty()
        && Path::new(trainer_path).is_file();

    let trainer_hash_valid = if trainer_present {
        let path = Path::new(trainer_path);
        verify_and_cache_trainer_hash(conn, profile_id, path)?.is_some()
    } else {
        false
    };

    let game_path = effective.game.executable_path.trim();
    let game_present = !game_path.is_empty() && Path::new(game_path).is_file();

    let proton_path = match resolved_method {
        METHOD_PROTON_RUN => {
            let runtime_proton = effective.runtime.proton_path.trim();
            if !runtime_proton.is_empty() {
                runtime_proton
            } else {
                // Backward-compatible fallback for older profiles that only stored steam.proton_path.
                effective.steam.proton_path.trim()
            }
        }
        METHOD_STEAM_APPLAUNCH => effective.steam.proton_path.trim(),
        _ => effective.runtime.proton_path.trim(),
    };
    let proton_available = !proton_path.is_empty() && Path::new(proton_path).exists();

    let prefix_path = match resolved_method {
        METHOD_PROTON_RUN => {
            let runtime_prefix = effective.runtime.prefix_path.trim();
            if !runtime_prefix.is_empty() {
                runtime_prefix
            } else {
                // Fallback to compatdata root if runtime.prefix_path has not been hydrated.
                effective.steam.compatdata_path.trim()
            }
        }
        METHOD_STEAM_APPLAUNCH => effective.steam.compatdata_path.trim(),
        _ => effective.runtime.prefix_path.trim(),
    };
    let prefix_exists = !prefix_path.is_empty() && Path::new(prefix_path).is_dir();

    let mut report = compute_offline_readiness(
        profile_name,
        tt_id,
        trainer_present,
        trainer_hash_valid,
        game_present,
        proton_available,
        prefix_exists,
        network_required,
        score_cap,
    );
    report.persist = OfflineReadinessPersistHints {
        trainer_present: i64::from(u8::from(trainer_present)),
        trainer_hash_valid: i64::from(u8::from(trainer_hash_valid)),
        trainer_activated: 0,
        proton_available: i64::from(u8::from(proton_available)),
        community_tap_cached: 0,
        network_required: i64::from(u8::from(network_required)),
    };
    Ok(report)
}

/// Append offline readiness checks to a profile health report and persist a snapshot row.
pub fn enrich_health_report_with_offline(
    conn: &Connection,
    profile_name: &str,
    profile_id: &str,
    profile: &GameProfile,
    report: &mut ProfileHealthReport,
) -> Result<Option<OfflineReadinessReport>, MetadataStoreError> {
    let effective = profile.effective_profile();
    if effective.trainer.path.trim().is_empty() {
        return Ok(None);
    }
    let off = check_offline_preflight(profile_name, profile_id, profile, conn)?;
    persist_offline_readiness_from_report(conn, profile_id, &off)?;
    for c in &off.checks {
        report.issues.push(HealthIssue {
            field: format!("offline_readiness.{}", c.field),
            path: c.path.clone(),
            message: c.message.clone(),
            remediation: c.remediation.clone(),
            severity: c.severity.clone(),
        });
    }
    Ok(Some(off))
}

pub fn persist_offline_readiness_from_report(
    conn: &Connection,
    profile_id: &str,
    report: &OfflineReadinessReport,
) -> Result<(), MetadataStoreError> {
    let blocking = if report.blocking_reasons.is_empty() {
        None
    } else {
        Some(report.blocking_reasons.join("; "))
    };
    offline_store::upsert_offline_readiness_snapshot(
        conn,
        profile_id,
        &report.readiness_state,
        i64::from(report.score),
        &report.trainer_type,
        report.persist.trainer_present,
        report.persist.trainer_hash_valid,
        report.persist.trainer_activated,
        report.persist.proton_available,
        report.persist.community_tap_cached,
        report.persist.network_required,
        blocking.as_deref(),
        &report.checked_at,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preflight_uses_runtime_paths_for_proton_run_profiles() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let db_path = tmp.path().join("meta.db");
        let conn = Connection::open(db_path).expect("open sqlite");

        let proton_exec = tmp.path().join("runtime_proton");
        std::fs::write(&proton_exec, b"#!/bin/sh\necho proton").expect("write proton");
        let prefix_dir = tmp.path().join("prefix");
        std::fs::create_dir_all(&prefix_dir).expect("create prefix");
        let game = tmp.path().join("game.exe");
        std::fs::write(&game, b"game").expect("write game");

        let mut profile = GameProfile::default();
        profile.launch.method = METHOD_PROTON_RUN.to_string();
        profile.steam.app_id = "123".to_string();
        profile.steam.proton_path.clear();
        profile.runtime.proton_path = proton_exec.to_string_lossy().to_string();
        profile.runtime.prefix_path = prefix_dir.to_string_lossy().to_string();
        profile.trainer.path.clear();
        profile.game.executable_path = game.to_string_lossy().to_string();

        let report = check_offline_preflight("p", "id-1", &profile, &conn).expect("preflight");
        let proton = report
            .checks
            .iter()
            .find(|c| c.field == "proton_available")
            .expect("proton check");
        let prefix = report
            .checks
            .iter()
            .find(|c| c.field == "prefix_exists")
            .expect("prefix check");
        assert!(matches!(proton.severity, HealthIssueSeverity::Info));
        assert!(matches!(prefix.severity, HealthIssueSeverity::Info));
    }

    #[test]
    fn preflight_uses_compatdata_for_steam_applaunch_prefix_check() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let db_path = tmp.path().join("meta.db");
        let conn = Connection::open(db_path).expect("open sqlite");

        let steam_proton = tmp.path().join("steam_proton");
        std::fs::write(&steam_proton, b"#!/bin/sh\necho proton").expect("write proton");
        let compatdata = tmp.path().join("compatdata").join("123");
        std::fs::create_dir_all(&compatdata).expect("create compatdata");
        let game = tmp.path().join("game.exe");
        std::fs::write(&game, b"game").expect("write game");

        let mut profile = GameProfile::default();
        profile.launch.method = METHOD_STEAM_APPLAUNCH.to_string();
        profile.steam.app_id = "123".to_string();
        profile.steam.proton_path = steam_proton.to_string_lossy().to_string();
        profile.steam.compatdata_path = compatdata.to_string_lossy().to_string();
        profile.runtime.prefix_path.clear();
        profile.trainer.path.clear();
        profile.game.executable_path = game.to_string_lossy().to_string();

        let report = check_offline_preflight("p", "id-2", &profile, &conn).expect("preflight");
        let prefix = report
            .checks
            .iter()
            .find(|c| c.field == "prefix_exists")
            .expect("prefix check");
        assert!(matches!(prefix.severity, HealthIssueSeverity::Info));
    }

    #[test]
    fn compute_all_pass_uncapped_is_100() {
        let r = compute_offline_readiness(
            "p",
            "standalone",
            true,
            true,
            true,
            true,
            true,
            false,
            None,
        );
        assert_eq!(r.score, 100);
        assert_eq!(r.readiness_state, "ready");
    }

    #[test]
    fn compute_trainer_missing_is_70() {
        let r = compute_offline_readiness(
            "p",
            "standalone",
            false,
            false,
            true,
            true,
            true,
            false,
            None,
        );
        assert_eq!(r.score, 70);
        assert_eq!(r.readiness_state, "unconfigured");
    }

    #[test]
    fn offline_readiness_report_serde_roundtrip() {
        let r = OfflineReadinessReport {
            profile_name: "x".to_string(),
            score: 55,
            readiness_state: "degraded".to_string(),
            trainer_type: "aurora".to_string(),
            checks: vec![],
            blocking_reasons: vec!["a".to_string()],
            checked_at: "2026-01-01T00:00:00+00:00".to_string(),
            persist: OfflineReadinessPersistHints::default(),
        };
        let j = serde_json::to_string(&r).unwrap();
        let back: OfflineReadinessReport = serde_json::from_str(&j).unwrap();
        assert_eq!(back.profile_name, r.profile_name);
        assert_eq!(back.score, r.score);
        assert_eq!(back.readiness_state, r.readiness_state);
        assert_eq!(back.trainer_type, r.trainer_type);
        assert_eq!(back.checked_at, r.checked_at);
    }
}
