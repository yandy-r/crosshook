//! Metadata store health IPC surface.

use super::{AppState, IpcResult};
use crosshook_core::metadata::MetadataStatus;

/// Reports metadata store health for the UI degraded-state banner and
/// diagnostics flows.
///
/// JSON shape is part of the IPC contract:
/// `{"state":"ok"}`, `{"state":"newer_schema","found":N,"supported":N}`,
/// `{"state":"disabled","reason":"..."}`.
#[tauri::command]
pub async fn metadata_store_status(app: AppState<'_>) -> IpcResult<MetadataStatus> {
    Ok(app.services().diagnostics.metadata_status())
}
