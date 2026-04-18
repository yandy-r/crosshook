#![cfg(test)]
//! Shared test fixtures for the metadata/tests/* split. Visibility is `pub(super)`
//! so siblings can import via `use super::test_support::*;`.

use rusqlite::{params, Connection};

use super::MetadataStore;
use crate::launch::diagnostics::models::{DiagnosticReport, ExitCodeInfo, FailureMode};
use crate::launch::request::ValidationSeverity;
use crate::profile::{
    GameProfile, GameSection, InjectionSection, LaunchSection, LauncherSection,
    LocalOverrideSection, RuntimeSection, SteamSection, TrainerLoadingMode, TrainerSection,
};

pub(super) fn sample_profile() -> GameProfile {
    GameProfile {
        game: GameSection {
            name: "Elden Ring".to_string(),
            executable_path: "/games/elden-ring/eldenring.exe".to_string(),
            custom_cover_art_path: String::new(),
            custom_portrait_art_path: String::new(),
            custom_background_art_path: String::new(),
        },
        trainer: TrainerSection {
            path: "/trainers/elden-ring.exe".to_string(),
            kind: "fling".to_string(),
            loading_mode: TrainerLoadingMode::SourceDirectory,
            trainer_type: "unknown".to_string(),
            required_protontricks: Vec::new(),
            community_trainer_sha256: String::new(),
        },
        injection: InjectionSection {
            dll_paths: vec!["/dlls/a.dll".to_string(), "/dlls/b.dll".to_string()],
            inject_on_launch: vec![true, false],
        },
        steam: SteamSection {
            enabled: true,
            app_id: "1245620".to_string(),
            compatdata_path: "/steam/compatdata/1245620".to_string(),
            proton_path: "/steam/proton/proton".to_string(),
            launcher: LauncherSection {
                icon_path: "/icons/elden-ring.png".to_string(),
                display_name: "Elden Ring".to_string(),
            },
        },
        runtime: RuntimeSection {
            prefix_path: String::new(),
            proton_path: String::new(),
            working_directory: String::new(),
            steam_app_id: String::new(),
            umu_game_id: String::new(),
            umu_preference: None,
        },
        launch: LaunchSection {
            method: "steam_applaunch".to_string(),
            ..Default::default()
        },
        local_override: LocalOverrideSection::default(),
    }
}

pub(super) fn connection(store: &MetadataStore) -> std::sync::MutexGuard<'_, Connection> {
    store
        .conn
        .as_ref()
        .expect("metadata store should expose a connection in tests")
        .lock()
        .expect("metadata store mutex should not be poisoned")
}

pub(super) fn clean_exit_report() -> DiagnosticReport {
    DiagnosticReport {
        severity: ValidationSeverity::Info,
        summary: "Clean exit".to_string(),
        exit_info: ExitCodeInfo {
            code: Some(0),
            signal: None,
            signal_name: None,
            core_dumped: false,
            failure_mode: FailureMode::CleanExit,
            description: "Process exited cleanly".to_string(),
            severity: ValidationSeverity::Info,
        },
        pattern_matches: vec![],
        suggestions: vec![],
        launch_method: "native".to_string(),
        log_tail_path: None,
        analyzed_at: "2026-01-01T00:00:00Z".to_string(),
    }
}

pub(super) fn insert_test_profile_row(conn: &Connection, profile_id: &str) {
    conn.execute(
        "INSERT INTO profiles (profile_id, current_filename, current_path, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            profile_id,
            format!("{profile_id}_file"),
            format!("/path/{profile_id}.toml"),
            "2024-01-01T00:00:00+00:00",
            "2024-01-01T00:00:00+00:00",
        ],
    )
    .unwrap();
}
