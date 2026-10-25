use crosshook_app::bootstrap::StartupReport;
use crosshook_app::BootstrappedApp;
use tauri::State;

/// Returns a live, idempotent startup snapshot, including completed job results.
#[tauri::command]
pub async fn get_startup_report(app: State<'_, BootstrappedApp>) -> Result<StartupReport, String> {
    let startup = app.startup();
    tauri::async_runtime::spawn_blocking(move || startup.snapshot())
        .await
        .map_err(|error| {
            tracing::warn!(%error, "startup report snapshot task failed");
            "Startup report is unavailable".to_string()
        })
}

#[cfg(test)]
mod tests;
