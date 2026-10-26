//! One-time import of Classic's library view mode into `[ui]` settings.
//!
//! Runs once during bootstrap, after settings load and logging init, and only
//! when no explicit view mode is stored yet. Best-effort: a missing or
//! unreadable WebKit store writes nothing.

use crosshook_core::settings::{webkit_import, AppSettingsData, LibraryViewMode, SettingsStore};

use super::StoreRoots;

/// Imports Classic's WebKit `library_view_mode` when eligible.
///
/// Skips unless roots are [`StoreRoots::Xdg`], settings were readable, and
/// `settings.ui.library_view_mode` is `None`. Adds no `BootstrapStep`.
pub(super) fn import_step(
    store: &SettingsStore,
    settings: &AppSettingsData,
    unreadable: bool,
    roots: &StoreRoots,
) {
    if unreadable {
        return;
    }
    if !matches!(roots, StoreRoots::Xdg) {
        return;
    }
    if settings.ui.library_view_mode.is_some() {
        return;
    }
    let imported = webkit_import::import_library_view_mode();
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
            tracing::warn!(%error, "failed to persist imported library view mode");
            false
        }
    }
}

#[cfg(test)]
mod tests;
