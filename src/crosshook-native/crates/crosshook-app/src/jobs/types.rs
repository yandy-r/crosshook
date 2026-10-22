//! Job identifiers, specs, and the events a job publishes.

use std::fmt;
use std::path::Path;

use crate::error::{AppError, AppErrorKind};
use crate::log_tail::LogBatch;

/// Unguessable job identifier: a UUID v4 string, the same shape as the
/// ProtonUp `op_id`. Only the caller that started a job (or one it handed the
/// id to) can address it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct JobId(String);

impl JobId {
    pub(crate) fn new_v4() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    /// Wraps an id received from a caller (e.g. over IPC). The registry
    /// answers `NotFound` for ids it never issued.
    pub fn from_string(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the id as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for JobId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Optional uniqueness key: at most one running job may hold a given key
/// (e.g. a Wine prefix path). See [`JobRegistry::start`](super::JobRegistry::start).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct JobKey(String);

impl JobKey {
    /// Creates a key from any string.
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    /// Creates a key from a path (lossy for non-UTF-8 bytes).
    pub fn from_path(path: &Path) -> Self {
        Self(path.to_string_lossy().into_owned())
    }

    /// Returns the key as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// What to start: a kind label for logs and an optional exclusivity key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobSpec {
    /// Short static label (e.g. `"install"`, `"run-executable"`).
    pub kind: &'static str,
    /// Exclusivity key; `None` means the job never conflicts.
    pub key: Option<JobKey>,
}

impl JobSpec {
    /// A spec with no exclusivity key.
    pub fn new(kind: &'static str) -> Self {
        Self { kind, key: None }
    }

    /// Sets the exclusivity key.
    #[must_use]
    pub fn with_key(mut self, key: JobKey) -> Self {
        self.key = Some(key);
        self
    }
}

/// Progress report published by a job.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct JobProgress {
    /// Units completed so far.
    pub current: u64,
    /// Total units, when known.
    pub total: Option<u64>,
    /// Optional human-readable status line.
    pub message: Option<String>,
}

/// Terminal result of a job.
///
/// Process exit codes are not modeled here: a job that needs to report a
/// non-zero exit maps it to [`Outcome::Failed`] with its own message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The job completed its work.
    Succeeded,
    /// The job failed. A panicking job yields an
    /// [`AppErrorKind::Internal`] error with a fixed message.
    Failed(AppError),
    /// The job stopped because it, the registry, or the app was cancelled,
    /// or its task was dropped by runtime shutdown.
    Cancelled,
}

impl From<Result<(), AppError>> for Outcome {
    /// `Ok` → `Succeeded`; an error of kind [`AppErrorKind::Cancelled`] →
    /// `Cancelled`; any other error → `Failed`.
    fn from(result: Result<(), AppError>) -> Self {
        match result {
            Ok(()) => Self::Succeeded,
            Err(error) if error.kind() == AppErrorKind::Cancelled => Self::Cancelled,
            Err(error) => Self::Failed(error),
        }
    }
}

/// Payload of a [`JobEvent`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobEventKind {
    /// Log output, or `LogBatch::Dropped { n }` when the receiver lagged and
    /// `n` events were lost.
    Log(LogBatch),
    /// Progress report.
    Progress(JobProgress),
    /// Terminal event; always the last event a receiver yields.
    Finished(Outcome),
}

/// One event published by a job. Every event carries its job's id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobEvent {
    /// Id of the job that published the event.
    pub job_id: JobId,
    /// Event payload.
    pub kind: JobEventKind,
}
