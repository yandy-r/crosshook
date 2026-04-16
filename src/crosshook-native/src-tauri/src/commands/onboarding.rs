use crosshook_core::metadata::MetadataStore;
use crosshook_core::onboarding::readiness::{
    apply_install_nag_dismissal, apply_readiness_nag_dismissals,
    apply_steam_deck_caveats_dismissal, check_generalized_readiness as eval_generalized_readiness,
    check_system_readiness,
};
use crosshook_core::onboarding::{
    global_readiness_catalog, ReadinessCheckResult, TrainerGuidanceContent, TrainerGuidanceEntry,
};
use crosshook_core::settings::SettingsStore;
use std::convert::Infallible;
use tauri::State;

use super::shared::sanitize_display_path;

fn require_readiness_metadata(metadata: &MetadataStore) -> Result<(), String> {
    if metadata.is_available() {
        Ok(())
    } else {
        Err(
            "readiness nag dismissals require the SQLite metadata store; dismiss_umu_install_nag and dismiss_steam_deck_caveats remain available via settings.toml".to_string(),
        )
    }
}

#[tauri::command]
pub fn check_readiness(
    store: State<'_, SettingsStore>,
    metadata: State<'_, MetadataStore>,
) -> Result<ReadinessCheckResult, String> {
    let settings = store.load().map_err(|e| e.to_string())?;
    let mut result = check_system_readiness();
    apply_install_nag_dismissal(&mut result, &settings.install_nag_dismissed_at);
    apply_steam_deck_caveats_dismissal(&mut result, &settings.steam_deck_caveats_dismissed_at);
    if metadata.is_available() {
        let dismissed = metadata
            .get_dismissed_readiness_nags()
            .map_err(|e| e.to_string())?;
        apply_readiness_nag_dismissals(&mut result, &dismissed);
    }
    for issue in &mut result.checks {
        issue.path = sanitize_display_path(&issue.path);
    }
    Ok(result)
}

#[tauri::command]
pub fn check_generalized_readiness(
    store: State<'_, SettingsStore>,
    metadata: State<'_, MetadataStore>,
) -> Result<ReadinessCheckResult, String> {
    let settings = store.load().map_err(|e| e.to_string())?;
    let catalog = global_readiness_catalog();
    let mut result = eval_generalized_readiness(catalog);
    // Persist probe-derived snapshot before dismissal overlays mutate the IPC payload.
    if metadata.is_available() {
        metadata
            .upsert_host_readiness_snapshot(
                &result.tool_checks,
                &result.detected_distro_family,
                result.all_passed,
                result.critical_failures,
                result.warnings,
            )
            .map_err(|e| e.to_string())?;
    }
    apply_install_nag_dismissal(&mut result, &settings.install_nag_dismissed_at);
    apply_steam_deck_caveats_dismissal(&mut result, &settings.steam_deck_caveats_dismissed_at);
    if metadata.is_available() {
        let dismissed = metadata
            .get_dismissed_readiness_nags()
            .map_err(|e| e.to_string())?;
        apply_readiness_nag_dismissals(&mut result, &dismissed);
    }
    for issue in &mut result.checks {
        issue.path = sanitize_display_path(&issue.path);
    }
    Ok(result)
}

#[tauri::command]
pub fn dismiss_onboarding(store: State<'_, SettingsStore>) -> Result<(), String> {
    let mut settings = store.load().map_err(|e| e.to_string())?;
    settings.onboarding_completed = true;
    store.save(&settings).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn dismiss_umu_install_nag(
    store: State<'_, SettingsStore>,
    metadata: State<'_, MetadataStore>,
) -> Result<(), String> {
    // Persist settings first so we never record metadata dismissal without the
    // legacy settings.toml timestamp (used by check_readiness / check_generalized_readiness).
    store
        .update(|settings| {
            settings.install_nag_dismissed_at = Some(chrono::Utc::now().to_rfc3339());
            Ok::<(), Infallible>(())
        })
        .map_err(|e| e.to_string())?
        .unwrap();
    if metadata.is_available() {
        metadata
            .dismiss_readiness_nag("umu_run", 3650)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn dismiss_steam_deck_caveats(
    store: State<'_, SettingsStore>,
    metadata: State<'_, MetadataStore>,
) -> Result<(), String> {
    store
        .update(|settings| {
            settings.steam_deck_caveats_dismissed_at = Some(chrono::Utc::now().to_rfc3339());
            Ok::<(), Infallible>(())
        })
        .map_err(|e| e.to_string())?
        .unwrap();
    if metadata.is_available() {
        metadata
            .dismiss_readiness_nag("steam_deck_caveats", 3650)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn dismiss_readiness_nag(
    metadata: State<'_, MetadataStore>,
    tool_id: String,
    ttl_days: Option<u32>,
) -> Result<(), String> {
    require_readiness_metadata(&metadata)?;
    let catalog = global_readiness_catalog();
    if catalog.find_by_id(&tool_id).is_none() {
        return Err(format!(
            "unknown readiness tool_id: {tool_id}. Use a tool id from the host readiness catalog."
        ));
    }
    metadata
        .dismiss_readiness_nag(&tool_id, ttl_days.unwrap_or(90))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_trainer_guidance() -> TrainerGuidanceContent {
    const SOURCE_DIRECTORY_DESC: &str =
        "Proton reads the trainer directly from its downloaded location. The trainer stays in place.";
    const COPY_TO_PREFIX_DESC: &str =
        "CrossHook copies the trainer and support files into the WINE prefix's C:\\ drive before launch.";
    const FLING_DESC: &str =
        "FLiNG standalone .exe trainers — free, no account required. Primary recommendation.";
    const WEMOD_DESC: &str =
        "WeMod extracted trainers — requires a WeMod account and the WeMod desktop app installed under WINE.";

    TrainerGuidanceContent {
        loading_modes: vec![
            TrainerGuidanceEntry {
                id: "source_directory".to_string(),
                title: "Source Directory".to_string(),
                description: SOURCE_DIRECTORY_DESC.to_string(),
                when_to_use: "Use when the trainer runs standalone without extra DLLs or support files.".to_string(),
                examples: vec!["FLiNG single-file .exe trainers".to_string()],
            },
            TrainerGuidanceEntry {
                id: "copy_to_prefix".to_string(),
                title: "Copy to Prefix".to_string(),
                description: COPY_TO_PREFIX_DESC.to_string(),
                when_to_use: "Use when the trainer bundles DLLs or support files that must be present in the WINE prefix.".to_string(),
                examples: vec![
                    "FLiNG trainers that bundle DLLs".to_string(),
                    "Trainers with companion .ini or .dat files".to_string(),
                ],
            },
        ],
        trainer_sources: vec![
            TrainerGuidanceEntry {
                id: "fling".to_string(),
                title: "FLiNG Trainers".to_string(),
                description: FLING_DESC.to_string(),
                when_to_use: "Primary recommendation — no account needed, direct .exe download.".to_string(),
                examples: vec!["flingtrainer.com standalone executables".to_string()],
            },
            TrainerGuidanceEntry {
                id: "wemod".to_string(),
                title: "WeMod".to_string(),
                description: WEMOD_DESC.to_string(),
                when_to_use: "Use only if WeMod is already set up under WINE. See wemod-launcher for setup instructions.".to_string(),
                examples: vec!["WeMod extracted trainer DLLs".to_string()],
            },
        ],
        verification_steps: vec![
            "Verify the trainer .exe file exists at the configured path.".to_string(),
            "Confirm the game version matches the trainer's target version.".to_string(),
            "For Copy to Prefix mode: ensure companion DLLs and support files are in the same directory.".to_string(),
            "Launch the game at least once to initialize the WINE prefix before using trainers.".to_string(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::Infallible;

    #[test]
    fn command_signatures_match_expected_ipc_contract() {
        let _ = check_readiness
            as fn(
                State<'_, SettingsStore>,
                State<'_, MetadataStore>,
            ) -> Result<ReadinessCheckResult, String>;
        let _ = check_generalized_readiness
            as fn(
                State<'_, SettingsStore>,
                State<'_, MetadataStore>,
            ) -> Result<ReadinessCheckResult, String>;
        let _ = dismiss_onboarding as fn(State<'_, SettingsStore>) -> Result<(), String>;
        let _ = dismiss_umu_install_nag
            as fn(State<'_, SettingsStore>, State<'_, MetadataStore>) -> Result<(), String>;
        let _ = dismiss_steam_deck_caveats
            as fn(State<'_, SettingsStore>, State<'_, MetadataStore>) -> Result<(), String>;
        let _ = dismiss_readiness_nag
            as fn(State<'_, MetadataStore>, String, Option<u32>) -> Result<(), String>;
        let _ = get_trainer_guidance as fn() -> TrainerGuidanceContent;
    }

    #[test]
    fn require_readiness_metadata_rejects_disabled_store() {
        let metadata = MetadataStore::disabled();
        let error = require_readiness_metadata(&metadata).expect_err(
            "disabled metadata store should surface an error so the frontend does not pretend dismissal succeeded",
        );
        assert!(
            error.contains("SQLite metadata store"),
            "error should explain why per-tool readiness dismissal could not be persisted"
        );
    }

    #[test]
    fn require_readiness_metadata_accepts_available_store() {
        let metadata = MetadataStore::open_in_memory().expect("in-memory metadata store");
        require_readiness_metadata(&metadata).expect("available metadata store should be accepted");
    }

    /// Exercises the underlying mutation that `dismiss_umu_install_nag` performs,
    /// without requiring Tauri State machinery.
    #[test]
    fn dismiss_umu_install_nag_updates_settings() {
        let tmp = tempfile::tempdir().unwrap();
        let store = SettingsStore::with_base_path(tmp.path().to_path_buf());

        let initial = store.load().unwrap();
        assert!(
            initial.install_nag_dismissed_at.is_none(),
            "should start with no dismiss timestamp"
        );

        store
            .update(|settings| {
                settings.install_nag_dismissed_at = Some(chrono::Utc::now().to_rfc3339());
                Ok::<(), Infallible>(())
            })
            .unwrap()
            .unwrap();

        let reloaded = store.load().unwrap();
        assert!(
            reloaded.install_nag_dismissed_at.is_some(),
            "dismiss_umu_install_nag must persist a non-None timestamp"
        );

        assert!(
            !reloaded.onboarding_completed,
            "install-nag dismissal must not affect onboarding_completed"
        );
    }

    /// Exercises the underlying mutation that `dismiss_steam_deck_caveats` performs,
    /// without requiring Tauri State machinery.
    #[test]
    fn dismiss_steam_deck_caveats_updates_settings() {
        let tmp = tempfile::tempdir().unwrap();
        let store = SettingsStore::with_base_path(tmp.path().to_path_buf());

        let initial = store.load().unwrap();
        assert!(
            initial.steam_deck_caveats_dismissed_at.is_none(),
            "should start with no dismiss timestamp"
        );

        store
            .update(|settings| {
                settings.steam_deck_caveats_dismissed_at = Some(chrono::Utc::now().to_rfc3339());
                Ok::<(), Infallible>(())
            })
            .unwrap()
            .unwrap();

        let reloaded = store.load().unwrap();
        assert!(
            reloaded.steam_deck_caveats_dismissed_at.is_some(),
            "dismiss_steam_deck_caveats must persist a non-None timestamp"
        );

        assert!(
            !reloaded.onboarding_completed,
            "caveats dismissal must not affect onboarding_completed"
        );
    }

    /// Verifies that `apply_steam_deck_caveats_dismissal` clears the caveats payload
    /// when a dismissal timestamp is present, mirroring the `apply_install_nag_dismissal` contract.
    #[test]
    fn check_readiness_applies_steam_deck_caveats_dismissal() {
        use crosshook_core::onboarding::SteamDeckCaveats;

        let mut result = crosshook_core::onboarding::check_system_readiness();
        result.steam_deck_caveats = Some(SteamDeckCaveats {
            description: "test caveats".to_string(),
            items: vec!["caveat one".to_string()],
            docs_url: "https://example.com".to_string(),
        });

        let dismissed_at = Some("2026-04-15T12:00:00Z".to_string());
        apply_steam_deck_caveats_dismissal(&mut result, &dismissed_at);
        assert!(
            result.steam_deck_caveats.is_none(),
            "apply_steam_deck_caveats_dismissal must clear steam_deck_caveats when dismissed"
        );

        result.steam_deck_caveats = Some(SteamDeckCaveats {
            description: "test caveats".to_string(),
            items: vec!["caveat one".to_string()],
            docs_url: "https://example.com".to_string(),
        });
        apply_steam_deck_caveats_dismissal(&mut result, &None);
        assert!(
            result.steam_deck_caveats.is_some(),
            "apply_steam_deck_caveats_dismissal must not clear steam_deck_caveats when not dismissed"
        );
    }
}
