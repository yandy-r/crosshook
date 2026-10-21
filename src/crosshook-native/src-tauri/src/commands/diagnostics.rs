use crosshook_app::services::{DiagnosticsRequest, DiagnosticsService};
use crosshook_core::export::diagnostics::DiagnosticBundleResult;
use tauri::State;

#[tauri::command]
pub async fn export_diagnostics(
    redact_paths: bool,
    output_dir: Option<String>,
    service: State<'_, DiagnosticsService>,
) -> Result<DiagnosticBundleResult, String> {
    let export = service
        .export(DiagnosticsRequest {
            redact_paths,
            output_dir: output_dir.map(std::path::PathBuf::from),
            attachments: Vec::new(),
        })
        .await
        .map_err(|error| error.message().to_string())?;
    Ok(DiagnosticBundleResult {
        archive_path: export.archive_path.to_string_lossy().into_owned(),
        summary: export.summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        // Async commands cannot be cast to fn pointers; naming them pins the
        // exported symbol (and thus the IPC command name).
        let _ = export_diagnostics;
    }
}
