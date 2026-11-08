use crosshook_app::services::DiagnosticsRequest;
use crosshook_core::export::diagnostics::DiagnosticBundleResult;

use super::{AppState, IpcResult};
use crate::error_map::ipc;

#[tauri::command]
pub async fn export_diagnostics(
    redact_paths: bool,
    output_dir: Option<String>,
    app: AppState<'_>,
) -> IpcResult<DiagnosticBundleResult> {
    let export = ipc(app
        .services()
        .diagnostics
        .export(DiagnosticsRequest {
            redact_paths,
            output_dir: output_dir.map(std::path::PathBuf::from),
            attachments: Vec::new(),
        })
        .await)?;
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
        let _ = export_diagnostics;
    }
}
