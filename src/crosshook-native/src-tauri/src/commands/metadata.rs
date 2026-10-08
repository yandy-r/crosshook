//! Metadata store health IPC surface.

use crosshook_core::metadata::{MetadataStatus, MetadataStore};
use tauri::State;

/// Reports metadata store health for the UI degraded-state banner and
/// diagnostics flows.
///
/// JSON shape is part of the IPC contract:
/// `{"state":"ok"}`, `{"state":"newer_schema","found":N,"supported":N}`,
/// `{"state":"disabled","reason":"..."}`.
#[tauri::command]
pub fn metadata_store_status(
    metadata_store: State<'_, MetadataStore>,
) -> Result<MetadataStatus, String> {
    Ok(metadata_store.status())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        let _ =
            metadata_store_status as fn(State<'_, MetadataStore>) -> Result<MetadataStatus, String>;
    }
}
