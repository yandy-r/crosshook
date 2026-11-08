use super::{AppState, IpcResult};
use crosshook_app::bootstrap::StartupReport;

/// Returns a live, idempotent startup snapshot, including completed job results.
#[tauri::command]
pub async fn get_startup_report(app: AppState<'_>) -> IpcResult<StartupReport> {
    let startup = app.startup();
    tauri::async_runtime::spawn_blocking(move || startup.snapshot())
        .await
        .map_err(|error| {
            tracing::warn!(%error, "startup report snapshot task failed");
            "Startup report is unavailable".to_string()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        let _ = get_startup_report;
    }
}
