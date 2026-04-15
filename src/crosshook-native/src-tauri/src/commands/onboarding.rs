use crosshook_core::onboarding::{
    apply_install_nag_dismissal, check_system_readiness, ReadinessCheckResult,
    TrainerGuidanceContent, TrainerGuidanceEntry,
};
use crosshook_core::settings::SettingsStore;
use tauri::State;

use super::shared::sanitize_display_path;

#[tauri::command]
pub fn check_readiness(store: State<'_, SettingsStore>) -> Result<ReadinessCheckResult, String> {
    let settings = store.load().map_err(|e| e.to_string())?;
    let mut result = check_system_readiness();
    apply_install_nag_dismissal(&mut result, &settings.install_nag_dismissed_at);
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
pub fn dismiss_umu_install_nag(store: State<'_, SettingsStore>) -> Result<(), String> {
    let mut settings = store.load().map_err(|e| e.to_string())?;
    settings.install_nag_dismissed_at = Some(chrono::Utc::now().to_rfc3339());
    store.save(&settings).map_err(|e| e.to_string())
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

    #[test]
    fn command_signatures_match_expected_ipc_contract() {
        let _ =
            check_readiness as fn(State<'_, SettingsStore>) -> Result<ReadinessCheckResult, String>;
        let _ = dismiss_onboarding as fn(State<'_, SettingsStore>) -> Result<(), String>;
        let _ = dismiss_umu_install_nag as fn(State<'_, SettingsStore>) -> Result<(), String>;
        let _ = get_trainer_guidance as fn() -> TrainerGuidanceContent;
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

        // Replicate the command body using the same store primitive
        let mut settings = store.load().unwrap();
        settings.install_nag_dismissed_at = Some(chrono::Utc::now().to_rfc3339());
        store.save(&settings).unwrap();

        let reloaded = store.load().unwrap();
        assert!(
            reloaded.install_nag_dismissed_at.is_some(),
            "dismiss_umu_install_nag must persist a non-None timestamp"
        );

        // Onboarding completion is independent of the install-nag dismissal
        assert!(
            !reloaded.onboarding_completed,
            "install-nag dismissal must not affect onboarding_completed"
        );
    }
}
