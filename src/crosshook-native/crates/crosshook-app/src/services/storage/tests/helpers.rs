//! Isolated filesystem inventory and metadata fixtures for storage tests.

use std::fs::{self, File, FileTimes};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::{GameProfile, ProfileStore};
use crosshook_core::storage::{PrefixCleanupTarget, PrefixCleanupTargetKind};
use tempfile::TempDir;

use crate::jobs::{JobEventKind, JobHandle, JobRegistry, Outcome};
use crate::services::storage::StorageService;

pub(super) struct Harness {
    pub(super) root: TempDir,
    pub(super) profiles: ProfileStore,
    pub(super) metadata: MetadataStore,
    pub(super) service: StorageService,
    pub(super) referenced: PathBuf,
}

impl Harness {
    pub(super) fn new(available: bool) -> Self {
        let root = tempfile::tempdir().unwrap();
        let profiles = ProfileStore::with_base_path(root.path().join("profiles"));
        let metadata = if available {
            MetadataStore::open_in_memory().unwrap()
        } else {
            MetadataStore::disabled()
        };
        let service =
            StorageService::new(profiles.clone(), metadata.clone(), JobRegistry::default());
        let referenced = create_prefix(&root.path().join("prefixes/active"));
        let mut profile = GameProfile::default();
        profile.game.name = "Active".into();
        profile.runtime.prefix_path = path_string(&referenced);
        profiles.save("Active", &profile).unwrap();
        Self {
            root,
            profiles,
            metadata,
            service,
            referenced,
        }
    }

    pub(super) fn orphan(&self) -> PrefixCleanupTarget {
        let prefix = create_prefix(&self.root.path().join("prefixes/orphan"));
        fs::write(prefix.join("drive_c/payload"), b"orphan contents").unwrap();
        PrefixCleanupTarget {
            kind: PrefixCleanupTargetKind::OrphanPrefix,
            resolved_prefix_path: path_string(&prefix),
            target_path: path_string(&prefix),
        }
    }

    pub(super) fn staged(&self, old: bool) -> PrefixCleanupTarget {
        let target = self
            .referenced
            .join("drive_c/CrossHook/StagedTrainers/old-trainer");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("trainer.exe"), b"trainer contents").unwrap();
        if old {
            File::open(&target)
                .unwrap()
                .set_times(
                    FileTimes::new().set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1)),
                )
                .unwrap();
        }
        PrefixCleanupTarget {
            kind: PrefixCleanupTargetKind::StaleStagedTrainer,
            resolved_prefix_path: path_string(&self.referenced),
            target_path: path_string(&target),
        }
    }
}

pub(super) fn create_prefix(path: &Path) -> PathBuf {
    fs::create_dir_all(path.join("drive_c/CrossHook")).unwrap();
    fs::canonicalize(path).unwrap()
}

pub(super) fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

pub(super) async fn finish(handle: JobHandle) -> Outcome {
    let mut events = handle.into_events();
    while let Some(event) = events.recv().await {
        if let JobEventKind::Finished(outcome) = event.kind {
            return outcome;
        }
    }
    panic!("job ended without a terminal event");
}

pub(super) fn execute_sql(metadata: &MetadataStore, sql: &str) {
    metadata
        .with_sqlite_conn("storage test fixture", |conn| {
            conn.execute_batch(sql).map_err(|source| {
                crosshook_core::metadata::MetadataStoreError::Database {
                    action: "storage test fixture",
                    source,
                }
            })
        })
        .unwrap();
}
