use crosshook_core::export::diagnostics::{
    export_diagnostic_bundle, DiagnosticBundleOptions, DiagnosticBundleResult,
};
use crosshook_core::profile::ProfileStore;
use crosshook_core::settings::SettingsStore;
use tauri::State;

#[tauri::command]
pub fn export_diagnostics(
    redact_paths: bool,
    output_dir: Option<String>,
    store: State<'_, ProfileStore>,
    settings_store: State<'_, SettingsStore>,
) -> Result<DiagnosticBundleResult, String> {
    let options = DiagnosticBundleOptions {
        redact_paths,
        output_dir: output_dir.map(std::path::PathBuf::from),
    };
    export_diagnostic_bundle(&store, &settings_store, &options).map_err(|error| error.to_string())
}
