//! One-time import of Classic's library view mode into `[ui]` settings.
//!
//! Runs once during bootstrap, after settings load and logging init, and only
//! when no explicit view mode is stored yet. Best-effort: a missing or
//! unreadable WebKit store writes nothing.

use std::path::Path;

use crosshook_core::settings::{webkit_import, AppSettingsData, LibraryViewMode, SettingsStore};

use crate::AppError;

/// Imports Classic's WebKit `library_view_mode` when eligible.
///
/// Skips unless the caller resolved a WebKit local-storage path (production
/// resolves it only for XDG store roots), settings were readable, and
/// `settings.ui.library_view_mode` is `None`. Adds no `BootstrapStep`.
pub(super) fn import_step(
    store: &SettingsStore,
    settings: &AppSettingsData,
    unreadable: bool,
    local_storage: Option<&Path>,
) {
    if unreadable || local_storage.is_none() || settings.ui.library_view_mode.is_some() {
        return;
    }
    // Unwrapping is safe: `local_storage.is_none()` returned above.
    let imported = webkit_import::read_library_view_mode(local_storage.unwrap());
    apply_import(store, imported);
}

/// Persists `Some(mode)` via `store.update`, re-checking inside the closure
/// that the key is still `None`. Writes nothing for `None`. Returns whether
/// it wrote.
pub(super) fn apply_import(store: &SettingsStore, imported: Option<LibraryViewMode>) -> bool {
    let Some(mode) = imported else {
        return false;
    };
    let outcome = store.update(|settings| {
        if settings.ui.library_view_mode.is_some() {
            return Err(());
        }
        settings.ui.library_view_mode = Some(mode);
        Ok(())
    });
    match outcome {
        Ok(Ok(())) => true,
        Ok(Err(())) => false,
        Err(error) => {
            tracing::warn!(
                error = AppError::from(error).message(),
                "failed to persist imported library view mode"
            );
            false
        }
    }
}

#[cfg(test)]
mod tests;
