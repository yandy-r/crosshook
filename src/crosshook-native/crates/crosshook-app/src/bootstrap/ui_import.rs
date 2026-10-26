//! One-time import of Classic's library view mode into `[ui]` settings.
//!
//! Runs once during bootstrap, after settings load and logging init, and only
//! when no explicit view mode is stored yet. Best-effort: a missing or
//! unreadable WebKit store writes nothing.

use std::path::PathBuf;

use crosshook_core::settings::{webkit_import, AppSettingsData, LibraryViewMode, SettingsStore};

use crate::AppError;

/// Imports Classic's WebKit `library_view_mode` when eligible.
///
/// Skips unless the caller resolved WebKit local-storage candidates (production
/// resolves them only for XDG store roots), settings were readable, and
/// `settings.ui.library_view_mode` is `None`. Adds no `BootstrapStep`.
pub(super) fn import_step(
    store: &SettingsStore,
    settings: &AppSettingsData,
    unreadable: bool,
    local_storage: &[PathBuf],
) {
    if unreadable || settings.ui.library_view_mode.is_some() {
        return;
    }
    let imported = local_storage
        .iter()
        .find_map(|path| webkit_import::read_library_view_mode(path));
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
