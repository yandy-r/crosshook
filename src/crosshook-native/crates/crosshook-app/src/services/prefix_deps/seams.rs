//! Injectable boundaries of the prefix-dependency service and their core
//! implementations.

use std::path::Path;

use crosshook_core::metadata::{MetadataStore, MetadataStoreError, PrefixVersionRestoreJournalRow};
use crosshook_core::platform::is_flatpak;
use crosshook_core::prefix_deps::detection::{detect_binary, resolve_winetricks_path};
use crosshook_core::prefix_deps::runner::restore_prefix_windows_version;
use crosshook_core::prefix_deps::{
    BinaryDetectionResult, PrefixVersionRestoreOutcome, PrefixVersionRestorePlan,
};

use crate::jobs::processes_using_prefix;
use crate::services::BoxFuture;

/// Host detection of winetricks/protontricks.
pub(crate) trait ToolProbe: Send + Sync + 'static {
    /// Detects the tool, honouring the settings override path.
    fn detect(&self, settings_path: &str) -> BinaryDetectionResult;
    /// `winetricks` on PATH, for the protontricks fallback.
    fn winetricks_on_path(&self) -> Option<String>;
}

/// Storage of the Windows-version restore journal.
pub(crate) trait JournalStore: Send + Sync + 'static {
    fn is_available(&self) -> bool;
    fn load(
        &self,
        prefix: &str,
    ) -> Result<Option<PrefixVersionRestoreJournalRow>, MetadataStoreError>;
    fn list(&self) -> Result<Vec<PrefixVersionRestoreJournalRow>, MetadataStoreError>;
    fn upsert_pending(
        &self,
        row: &PrefixVersionRestoreJournalRow,
    ) -> Result<(), MetadataStoreError>;
    fn mark_failed(&self, prefix: &str, error: &str) -> Result<(), MetadataStoreError>;
    fn delete(&self, prefix: &str) -> Result<(), MetadataStoreError>;
    /// Atomically moves the row of `from` to `to` (a moved prefix).
    fn rekey(&self, from: &str, to: &str) -> Result<(), MetadataStoreError>;
}

/// Restores and verifies a prefix's Windows version.
pub(crate) trait PrefixVersionRestorer: Send + Sync + 'static {
    fn restore(
        &self,
        plan: PrefixVersionRestorePlan,
    ) -> BoxFuture<'static, PrefixVersionRestoreOutcome>;
}

/// Whether a live process uses a prefix.
pub(crate) trait PrefixInUse: Send + Sync + 'static {
    fn in_use(&self, canonical_prefix: &Path) -> bool;
}

/// Core host detection.
pub(crate) struct CoreToolProbe;

impl ToolProbe for CoreToolProbe {
    fn detect(&self, settings_path: &str) -> BinaryDetectionResult {
        detect_binary(settings_path)
    }

    fn winetricks_on_path(&self) -> Option<String> {
        resolve_winetricks_path()
    }
}

/// The metadata-store journal table.
pub(crate) struct MetadataJournalStore(pub(crate) MetadataStore);

impl JournalStore for MetadataJournalStore {
    fn is_available(&self) -> bool {
        self.0.is_available()
    }

    fn load(
        &self,
        prefix: &str,
    ) -> Result<Option<PrefixVersionRestoreJournalRow>, MetadataStoreError> {
        self.0.load_prefix_version_restore(prefix)
    }

    fn list(&self) -> Result<Vec<PrefixVersionRestoreJournalRow>, MetadataStoreError> {
        self.0.list_prefix_version_restores()
    }

    fn upsert_pending(
        &self,
        row: &PrefixVersionRestoreJournalRow,
    ) -> Result<(), MetadataStoreError> {
        self.0.upsert_prefix_version_restore(row)
    }

    fn mark_failed(&self, prefix: &str, error: &str) -> Result<(), MetadataStoreError> {
        self.0.mark_prefix_version_restore_failed(prefix, error)
    }

    fn delete(&self, prefix: &str) -> Result<(), MetadataStoreError> {
        self.0.delete_prefix_version_restore(prefix)
    }

    fn rekey(&self, from: &str, to: &str) -> Result<(), MetadataStoreError> {
        self.0.rekey_prefix_version_restore(from, to)
    }
}

/// Core restore + `system.reg` verification (90 s timeout).
pub(crate) struct CoreRestorer;

impl PrefixVersionRestorer for CoreRestorer {
    fn restore(
        &self,
        plan: PrefixVersionRestorePlan,
    ) -> BoxFuture<'static, PrefixVersionRestoreOutcome> {
        Box::pin(async move { restore_prefix_windows_version(&plan).await })
    }
}

/// `/proc/*/environ` scan for processes whose `WINEPREFIX` or
/// `STEAM_COMPAT_DATA_PATH` is the prefix (canonical path comparison).
///
/// Under Flatpak host processes are invisible to the sandbox `/proc`, so the
/// probe fails closed: the prefix is reported in use and its row waits for an
/// explicit repair.
pub(crate) struct ProcPrefixInUse;

impl PrefixInUse for ProcPrefixInUse {
    fn in_use(&self, canonical_prefix: &Path) -> bool {
        prefix_in_use_with(canonical_prefix, is_flatpak())
    }
}

/// [`ProcPrefixInUse`] with an explicit Flatpak decision.
pub(crate) fn prefix_in_use_with(canonical_prefix: &Path, flatpak: bool) -> bool {
    flatpak || !processes_using_prefix(canonical_prefix).is_empty()
}
