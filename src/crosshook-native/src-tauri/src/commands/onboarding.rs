use crosshook_core::onboarding::{
    check_system_readiness, ReadinessCheckResult, TrainerGuidanceContent, TrainerGuidanceEntry,
};
use crosshook_core::settings::SettingsStore;
use tauri::State;

use super::shared::sanitize_display_path;

#[tauri::command]
pub fn check_readiness() -> Result<ReadinessCheckResult, String> {
    let mut result = check_system_readiness();
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
        let _ = check_readiness as fn() -> Result<ReadinessCheckResult, String>;
        let _ = dismiss_onboarding as fn(State<'_, SettingsStore>) -> Result<(), String>;
        let _ = get_trainer_guidance as fn() -> TrainerGuidanceContent;
    }
}
