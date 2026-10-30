//! Argument-asserting fakes of the prefix-deps seams. Each fake panics when
//! asked about a prefix other than the one under test, so a test cannot pass
//! while the code addresses the wrong prefix.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use crosshook_core::metadata::{MetadataStore, MetadataStoreError, PrefixVersionRestoreJournalRow};
use crosshook_core::prefix_deps::{
    BinaryDetectionResult, PrefixDepsTool, PrefixVersionRestoreOutcome, PrefixVersionRestorePlan,
};

use super::super::seams::{
    JournalStore, MetadataJournalStore, PrefixInUse, PrefixVersionRestorer, ToolProbe,
};
use crate::services::BoxFuture;

/// Journal store over the real table with fault injection and a call log.
/// Panics when asked about any prefix other than the expected one.
pub(crate) struct RecordingStore {
    inner: MetadataJournalStore,
    expected: String,
    pub calls: Mutex<Vec<String>>,
    pub available: AtomicBool,
    pub fail_upsert: AtomicBool,
    pub fail_delete: AtomicBool,
    pub fail_mark: AtomicBool,
    /// Loads after this many successful ones report no row.
    pub vanish_after_loads: Mutex<Option<usize>>,
    loads: AtomicUsize,
}

impl RecordingStore {
    pub(crate) fn new(metadata: MetadataStore, expected: &str) -> Self {
        Self {
            inner: MetadataJournalStore(metadata),
            expected: expected.to_string(),
            calls: Mutex::new(Vec::new()),
            available: AtomicBool::new(true),
            fail_upsert: AtomicBool::new(false),
            fail_delete: AtomicBool::new(false),
            fail_mark: AtomicBool::new(false),
            vanish_after_loads: Mutex::new(None),
            loads: AtomicUsize::new(0),
        }
    }

    fn log(&self, op: &str, prefix: &str) {
        assert_eq!(
            prefix, self.expected,
            "journal store asked about a foreign prefix"
        );
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(op.to_string());
    }

    /// Operation names in call order (`load`, `delete`, `mark_failed:<error>`…).
    pub(crate) fn ops(&self) -> Vec<String> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn injected(flag: &AtomicBool, action: &str) -> Result<(), MetadataStoreError> {
        if flag.load(Ordering::SeqCst) {
            return Err(MetadataStoreError::Validation(format!(
                "injected {action} failure"
            )));
        }
        Ok(())
    }
}

impl JournalStore for RecordingStore {
    fn is_available(&self) -> bool {
        self.available.load(Ordering::SeqCst)
    }

    fn load(
        &self,
        prefix: &str,
    ) -> Result<Option<PrefixVersionRestoreJournalRow>, MetadataStoreError> {
        self.log("load", prefix);
        let seen = self.loads.fetch_add(1, Ordering::SeqCst);
        let vanish = *self
            .vanish_after_loads
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if vanish.is_some_and(|limit| seen >= limit) {
            return Ok(None);
        }
        self.inner.load(prefix)
    }

    fn list(&self) -> Result<Vec<PrefixVersionRestoreJournalRow>, MetadataStoreError> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push("list".to_string());
        self.inner.list()
    }

    fn upsert_pending(
        &self,
        row: &PrefixVersionRestoreJournalRow,
    ) -> Result<(), MetadataStoreError> {
        self.log("upsert", &row.prefix_path);
        assert_eq!(row.state, "pending");
        Self::injected(&self.fail_upsert, "upsert")?;
        self.inner.upsert_pending(row)
    }

    fn mark_failed(&self, prefix: &str, error: &str) -> Result<(), MetadataStoreError> {
        self.log(&format!("mark_failed:{error}"), prefix);
        Self::injected(&self.fail_mark, "mark")?;
        self.inner.mark_failed(prefix, error)
    }

    fn rekey(&self, from: &str, to: &str) -> Result<(), MetadataStoreError> {
        self.log("rekey", to);
        self.inner.rekey(from, to)
    }

    fn delete(&self, prefix: &str) -> Result<(), MetadataStoreError> {
        self.log("delete", prefix);
        Self::injected(&self.fail_delete, "delete")?;
        self.inner.delete(prefix)
    }
}

/// Restorer wrapper counting calls; asserts every plan targets the prefix.
pub(crate) struct CountingRestorer {
    inner: Arc<dyn PrefixVersionRestorer>,
    expected: String,
    pub calls: AtomicUsize,
}

impl CountingRestorer {
    pub(crate) fn new(inner: Arc<dyn PrefixVersionRestorer>, expected: &str) -> Self {
        Self {
            inner,
            expected: expected.to_string(),
            calls: AtomicUsize::new(0),
        }
    }

    pub(crate) fn count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl PrefixVersionRestorer for CountingRestorer {
    fn restore(
        &self,
        plan: PrefixVersionRestorePlan,
    ) -> BoxFuture<'static, PrefixVersionRestoreOutcome> {
        assert_eq!(
            plan.prefix_path, self.expected,
            "restore of a foreign prefix"
        );
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.inner.restore(plan)
    }
}

/// Restorer returning a fixed outcome (fault injection only).
pub(crate) struct FixedRestorer(pub PrefixVersionRestoreOutcome);

impl PrefixVersionRestorer for FixedRestorer {
    fn restore(
        &self,
        _plan: PrefixVersionRestorePlan,
    ) -> BoxFuture<'static, PrefixVersionRestoreOutcome> {
        let outcome = self.0.clone();
        Box::pin(async move { outcome })
    }
}

/// In-use probe with a fixed answer; asserts it is asked about the prefix.
pub(crate) struct FakeInUse {
    answer: bool,
    expected: PathBuf,
    pub calls: AtomicUsize,
}

impl FakeInUse {
    pub(crate) fn new(answer: bool, expected: &str) -> Self {
        Self {
            answer,
            expected: PathBuf::from(expected),
            calls: AtomicUsize::new(0),
        }
    }
}

impl PrefixInUse for FakeInUse {
    fn in_use(&self, canonical_prefix: &Path) -> bool {
        assert_eq!(
            canonical_prefix, self.expected,
            "in-use probe of a foreign prefix"
        );
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.answer
    }
}

/// Tool probe with a fixed detection and PATH lookup; records the settings
/// override paths it was asked with.
pub(crate) struct FakeProbe {
    pub detection: BinaryDetectionResult,
    pub winetricks: Option<String>,
    pub settings_paths: Mutex<Vec<String>>,
}

impl FakeProbe {
    /// Detection finding `binary` as winetricks; no PATH fallback.
    pub(crate) fn winetricks(binary: &str) -> Self {
        Self::found(binary, PrefixDepsTool::Winetricks, None)
    }

    pub(crate) fn found(binary: &str, tool: PrefixDepsTool, winetricks: Option<String>) -> Self {
        Self {
            detection: detection(Some(binary), Some(tool)),
            winetricks,
            settings_paths: Mutex::new(Vec::new()),
        }
    }

    /// Detection finding nothing.
    pub(crate) fn none() -> Self {
        Self {
            detection: detection(None, None),
            winetricks: None,
            settings_paths: Mutex::new(Vec::new()),
        }
    }
}

impl ToolProbe for FakeProbe {
    fn detect(&self, settings_path: &str) -> BinaryDetectionResult {
        self.settings_paths
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(settings_path.to_string());
        self.detection.clone()
    }

    fn winetricks_on_path(&self) -> Option<String> {
        self.winetricks.clone()
    }
}

/// A detection result for `binary` (not found when `None`).
pub(crate) fn detection(
    binary: Option<&str>,
    tool: Option<PrefixDepsTool>,
) -> BinaryDetectionResult {
    BinaryDetectionResult {
        found: binary.is_some(),
        binary_path: binary.map(str::to_string),
        binary_name: binary
            .and_then(|path| Path::new(path).file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
        tool_type: tool,
        source: if binary.is_some() {
            "settings"
        } else {
            "not_found"
        }
        .to_string(),
    }
}
