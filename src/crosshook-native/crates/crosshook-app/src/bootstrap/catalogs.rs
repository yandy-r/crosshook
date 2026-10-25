//! Catalog initialization and persistence.
//!
//! Phase B loads each embedded-default-plus-user-override catalog from the
//! settings config directory and installs it as the process-global catalog
//! (`OnceLock::set` semantics: first wins, so repeated bootstrap in tests is
//! idempotent). When the metadata store is healthy, the optimization and
//! readiness catalogs are then persisted to SQLite by [`persist`].
//!
//! Moved from `src-tauri/src/lib.rs::run()` / `setup()`.

use crosshook_core::launch::{
    global_catalog, initialize_catalog, initialize_command_argument_catalog, load_catalog,
    load_command_argument_catalog,
};
use crosshook_core::metadata::{MetadataStatus, MetadataStore};
use crosshook_core::offline::{initialize_trainer_type_catalog, load_trainer_type_catalog};
use crosshook_core::onboarding::{
    global_readiness_catalog, initialize_capability_map, initialize_readiness_catalog,
    load_capability_map, load_readiness_catalog,
};
use crosshook_core::settings::SettingsStore;

/// Loads and installs all process-global catalogs. User overrides resolve
/// relative to the settings store's config directory.
pub(crate) fn initialize(settings: &SettingsStore) {
    // Optimization catalog: embedded default → user override.
    let catalog = load_catalog(Some(&settings.base_path), &[]);
    initialize_catalog(catalog);

    // Command-argument catalog: embedded default → optional user override
    // only (no SQLite).
    let command_argument_catalog = load_command_argument_catalog(Some(&settings.base_path), &[]);
    initialize_command_argument_catalog(command_argument_catalog);

    let trainer_type_catalog = load_trainer_type_catalog(Some(&settings.base_path), &[]);
    initialize_trainer_type_catalog(trainer_type_catalog);

    let readiness_catalog = load_readiness_catalog(Some(&settings.base_path));
    initialize_readiness_catalog(readiness_catalog);

    let capability_map = load_capability_map(Some(&settings.base_path));
    initialize_capability_map(capability_map);
}

/// Persists the optimization and readiness catalogs to the metadata database.
/// No-op with a logged warning when the metadata store is not writable
/// (disabled or newer-schema); persistence errors are logged, never fatal.
pub(crate) fn persist(metadata: &MetadataStore) {
    if metadata.status() != MetadataStatus::Ok {
        tracing::warn!(
            status = ?metadata.status(),
            "metadata read-only or unavailable; catalog persistence skipped"
        );
        return;
    }

    let catalog = global_catalog();
    if let Err(error) =
        metadata.persist_optimization_catalog(&catalog.entries, catalog.catalog_version)
    {
        tracing::warn!(%error, "failed to persist optimization catalog to metadata db");
    }

    let readiness = global_readiness_catalog();
    if let Err(error) =
        metadata.persist_readiness_catalog(&readiness.entries, readiness.catalog_version)
    {
        tracing::warn!(%error, "failed to persist readiness catalog to metadata db");
    }
}

#[cfg(test)]
mod tests;
