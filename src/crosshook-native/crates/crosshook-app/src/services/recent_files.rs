//! Recent-files service over `recent.toml`, capped by the settings limit.

use std::sync::{Arc, Mutex, PoisonError};

use crosshook_core::settings::{
    clamp_recent_files_limit, RecentFilesData, RecentFilesStore, SettingsStore,
};

use super::run_blocking;
use crate::AppError;

/// Which recent-files list an entry belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecentFileKind {
    /// Game executables.
    Game,
    /// Trainer executables.
    Trainer,
    /// Injected DLLs.
    Dll,
}

impl RecentFileKind {
    fn list(self, data: &mut RecentFilesData) -> &mut Vec<String> {
        match self {
            Self::Game => &mut data.game_paths,
            Self::Trainer => &mut data.trainer_paths,
            Self::Dll => &mut data.dll_paths,
        }
    }
}

/// Recent-files service. Cheap to clone; clones share one I/O lock, so
/// load/save/push through any clone are serialized. Writers outside this
/// service (other processes or separately constructed services) are not.
#[derive(Debug, Clone)]
pub struct RecentFilesService {
    settings: SettingsStore,
    store: RecentFilesStore,
    io_lock: Arc<Mutex<()>>,
}

/// Effective per-list cap from the configured `recent_files_limit`.
fn recent_files_cap(settings: &SettingsStore) -> Result<usize, AppError> {
    let limit = settings.load()?.recent_files_limit;
    Ok(clamp_recent_files_limit(limit) as usize)
}

impl RecentFilesService {
    /// Creates a service; `settings` supplies the per-list cap.
    pub fn new(settings: SettingsStore, store: RecentFilesStore) -> Self {
        Self {
            settings,
            store,
            io_lock: Arc::new(Mutex::new(())),
        }
    }

    /// Runs `work(store, cap)` on the blocking pool while holding the shared
    /// I/O lock. A poisoned lock is recovered: the guarded data is `()`, and
    /// every write is atomic on disk.
    async fn with_store<T, F>(&self, work: F) -> Result<T, AppError>
    where
        F: FnOnce(&RecentFilesStore, usize) -> Result<T, AppError> + Send + 'static,
        T: Send + 'static,
    {
        let service = self.clone();
        run_blocking(move || {
            let _guard = service
                .io_lock
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            let cap = recent_files_cap(&service.settings)?;
            work(&service.store, cap)
        })
        .await
    }

    /// Loads recent files (missing paths dropped, lists capped). Unknown
    /// on-disk keys stay on disk and are not returned.
    pub async fn load(&self) -> Result<RecentFilesData, AppError> {
        self.with_store(|store, cap| {
            let mut recent = store.load(cap)?;
            recent.extra.clear();
            Ok(recent)
        })
        .await
    }

    /// Saves the known lists, preserving unknown on-disk keys; lists are
    /// capped and any unknown keys in `data` are ignored.
    pub async fn save(&self, data: RecentFilesData) -> Result<(), AppError> {
        self.with_store(move |store, cap| {
            store.save_from_ipc(&data, cap)?;
            Ok(())
        })
        .await
    }

    /// Moves `path` (trimmed) to the front of the `kind` list, removing
    /// duplicates and capping the list. Returns the updated lists.
    ///
    /// Empty or whitespace paths are rejected with a validation error.
    pub async fn push(
        &self,
        kind: RecentFileKind,
        path: &str,
    ) -> Result<RecentFilesData, AppError> {
        let path = path.trim().to_string();
        if path.is_empty() {
            return Err(AppError::validation("path", "recent file path is empty"));
        }
        self.with_store(move |store, cap| {
            let mut recent = store.load(cap)?;
            let list = kind.list(&mut recent);
            list.retain(|existing| existing.trim() != path);
            list.insert(0, path);
            // `save` applies the cap; `load` keeps unknown keys for the write.
            store.save(&recent, cap)?;
            recent.extra.clear();
            kind.list(&mut recent).truncate(cap);
            Ok(recent)
        })
        .await
    }
}

#[cfg(test)]
mod tests;
