use crosshook_core::platform::{
    resolve_picked_host_path_default, resolve_picked_sandbox_path, PickedPathUse,
};

/// Resolves a path returned by a file or folder picker for its declared use.
///
/// `Host` picks are strictly resolved to a host path (document-portal paths
/// included) or fail with a user-facing message; `Sandbox` picks keep their
/// document-portal path and only get the `/run/host` repair.
#[tauri::command]
pub async fn normalize_host_path(path: String, path_use: PickedPathUse) -> Result<String, String> {
    match path_use {
        PickedPathUse::Host => resolve_picked_host_path_default(&path)
            .await
            .map(|resolved| resolved.to_string_lossy().into_owned())
            .map_err(|error| error.to_string()),
        PickedPathUse::Sandbox => Ok(resolve_picked_sandbox_path(&path)),
    }
}
