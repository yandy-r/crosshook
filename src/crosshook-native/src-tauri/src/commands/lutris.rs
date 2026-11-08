//! Thin IPC adapters over the Lutris import service.

use std::path::PathBuf;

use crosshook_core::profile::{LutrisImportEntry, LutrisImportPreview, LutrisImportResult};

use super::{AppState, IpcResult};
use crate::error_map::ipc;

#[tauri::command]
pub async fn lutris_prepare_import(
    directory: Option<String>,
    app: AppState<'_>,
) -> IpcResult<LutrisImportPreview> {
    ipc(app
        .services()
        .lutris
        .prepare_import(directory.map(PathBuf::from))
        .await)
}

#[tauri::command]
pub async fn lutris_import_profiles(
    entries: Vec<LutrisImportEntry>,
    app: AppState<'_>,
) -> IpcResult<LutrisImportResult> {
    ipc(app.services().lutris.import_profiles(entries).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        let _ = lutris_prepare_import;
        let _ = lutris_import_profiles;
    }
}
