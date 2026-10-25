//! Persistent store construction and the service bundle built on top.
//!
//! Construction order is fixed by the Phase-B contract: the settings store
//! first ([`init_settings`]) so logging can start before the remaining stores,
//! then [`init_remaining`] for profiles, recent files, community taps and the
//! metadata store. Metadata failure is degraded, never fatal: the store falls
//! back to [`MetadataStore::disabled`] and the sanitized reason is returned
//! for the startup report.
//!
//! [`StoreRoots::Explicit`] is honored by every constructor (no XDG
//! fallback), so tests and portable runs never touch real user directories.

use std::sync::Arc;

use crosshook_core::community::CommunityTapStore;
use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::ProfileStore;
use crosshook_core::settings::{AppSettingsData, RecentFilesStore, SettingsStore};

use super::options::StoreRoots;
use super::{BootstrapError, StoreKind};
use crate::services::{
    BackgroundGrantHolder, DiagnosticsService, HealthService, RecentFilesService, SettingsService,
};

/// The persistent stores the application owns. Clones are cheap; clones of
/// each store share its on-disk location and I/O locking.
#[derive(Clone)]
pub struct Stores {
    /// User preferences (`settings.toml`).
    pub settings: SettingsStore,
    /// Game profiles.
    pub profiles: ProfileStore,
    /// Recently used files (`recent.toml`).
    pub recent_files: RecentFilesStore,
    /// Community tap subscriptions.
    pub community_taps: CommunityTapStore,
    /// SQLite metadata, health and history database.
    pub metadata: MetadataStore,
}

/// Toolkit-independent services built over [`Stores`]. Clones are cheap.
#[derive(Clone)]
pub struct Services {
    /// Typed settings reads and saves.
    pub settings: SettingsService,
    /// Recent-files lists.
    pub recent_files: RecentFilesService,
    /// Diagnostic bundle export.
    pub diagnostics: DiagnosticsService,
    /// Profile health summaries and cached snapshots.
    pub health: HealthService,
    /// Background-protection portal grant holder.
    pub background: Arc<BackgroundGrantHolder>,
}

impl Services {
    /// Builds the service bundle over `stores`.
    pub fn new(stores: &Stores) -> Self {
        Self {
            settings: SettingsService::new(
                stores.settings.clone(),
                stores.profiles.base_path.clone(),
            ),
            recent_files: RecentFilesService::new(
                stores.settings.clone(),
                stores.recent_files.clone(),
            ),
            diagnostics: DiagnosticsService::new(
                stores.profiles.clone(),
                stores.settings.clone(),
                stores.metadata.clone(),
            ),
            health: HealthService::new(stores.profiles.clone(), stores.metadata.clone()),
            background: Arc::new(BackgroundGrantHolder::new()),
        }
    }
}

/// Initializes the settings store. Fatal on failure (the caller converts the
/// error into a user-facing dialog); created first so logging can start
/// before the remaining stores are built.
///
/// An unusable config *root* is fatal ([`BootstrapError::StoreInit`]); an
/// unreadable settings *file* inside a usable root is not — that degrades to
/// default settings with `settings_unreadable` in the report.
pub(crate) fn init_settings(roots: &StoreRoots) -> Result<SettingsStore, BootstrapError> {
    let store = match roots {
        StoreRoots::Xdg => {
            SettingsStore::try_new().map_err(|detail| store_init(StoreKind::Settings, detail))?
        }
        StoreRoots::Explicit { config_dir, .. } => {
            SettingsStore::with_base_path(config_dir.clone())
        }
    };
    // Validate and create the root now so a file-as-root (or otherwise
    // unusable) config directory fails here instead of surfacing as a later
    // load/save error.
    ensure_directory_root(&store.base_path, StoreKind::Settings)?;
    Ok(store)
}

/// Initializes the remaining stores after settings and logging are up.
///
/// Returns the store bundle plus the sanitized metadata-unavailable detail
/// (`None` when the metadata store opened successfully). Profiles, recent
/// files and community taps are fatal on failure; metadata degrades to
/// [`MetadataStore::disabled`] instead.
pub(crate) fn init_remaining(
    roots: &StoreRoots,
    settings: &SettingsStore,
    settings_data: &AppSettingsData,
) -> Result<(Stores, Option<String>), BootstrapError> {
    // Same constructor for both roots so a settings-configured profiles
    // directory is honored; the fallback root is `<config>/profiles`.
    let profiles = ProfileStore::try_new_with_settings_data(settings_data, &settings.base_path)
        .map_err(|detail| store_init(StoreKind::Profiles, detail))?;

    let recent_files = match roots {
        StoreRoots::Xdg => RecentFilesStore::try_new()
            .map_err(|detail| store_init(StoreKind::RecentFiles, detail))?,
        StoreRoots::Explicit { data_dir, .. } => {
            let store = RecentFilesStore::with_path(data_dir.join("recent.toml"));
            // Fatal parity with profiles: an unusable data root must fail here,
            // not on the first user write.
            ensure_directory_root(data_dir, StoreKind::RecentFiles)?;
            store
        }
    };

    let community_taps = match roots {
        StoreRoots::Xdg => CommunityTapStore::try_new()
            .map_err(|detail| store_init(StoreKind::CommunityTaps, detail))?,
        StoreRoots::Explicit { data_dir, .. } => {
            ensure_directory_root(data_dir, StoreKind::CommunityTaps)?;
            CommunityTapStore::with_base_path(data_dir.join("community").join("taps"))
        }
    };

    let (metadata, metadata_error) = match roots {
        StoreRoots::Xdg => match MetadataStore::try_new() {
            Ok(store) => (store, None),
            Err(error) => {
                tracing::warn!(%error, "metadata store unavailable — SQLite features disabled");
                (
                    MetadataStore::disabled(),
                    Some(crate::presenter::sanitize_message(&error)),
                )
            }
        },
        StoreRoots::Explicit { data_dir, .. } => {
            let path = data_dir.join("metadata.db");
            match MetadataStore::with_path(&path) {
                Ok(store) => (store, None),
                Err(error) => {
                    tracing::warn!(%error, "metadata store unavailable — SQLite features disabled");
                    (
                        MetadataStore::disabled(),
                        Some(crate::presenter::sanitize_message(&error.to_string())),
                    )
                }
            }
        }
    };

    Ok((
        Stores {
            settings: settings.clone(),
            profiles,
            recent_files,
            community_taps,
            metadata,
        },
        metadata_error,
    ))
}

/// Fatal validation that `dir` is (or can become) an accessible directory.
/// The message names the failing root by store kind (`settings` →
/// configuration directory, everything else → data directory). An existing
/// directory is probed with `read_dir` so an inaccessible root (e.g. the
/// manual `chmod 000` scenario) fails here instead of on first write.
fn ensure_directory_root(dir: &std::path::Path, store: StoreKind) -> Result<(), BootstrapError> {
    let label = match store {
        StoreKind::Settings => "configuration directory",
        _ => "data directory",
    };
    if dir.is_dir() {
        return std::fs::read_dir(dir).map(|_| ()).map_err(|error| {
            store_init(
                store,
                format!("{label} {} is not accessible: {error}", dir.display()),
            )
        });
    }
    if dir.exists() {
        return Err(store_init(
            store,
            format!("{label} {} is not a directory", dir.display()),
        ));
    }
    std::fs::create_dir_all(dir)
        .map_err(|error| store_init(store, format!("{label} {}: {error}", dir.display())))
}

fn store_init(store: StoreKind, detail: String) -> BootstrapError {
    BootstrapError::StoreInit { store, detail }
}

#[cfg(test)]
mod tests;
