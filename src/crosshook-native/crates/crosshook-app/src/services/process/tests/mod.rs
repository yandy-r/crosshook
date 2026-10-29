//! Tests for the shared process plumbing.

mod child;
mod owner;
mod run;

use super::{
    conflict_as, prefix_job_key, prefix_job_key_for_path, MSG_PREFIX_BUSY, PREFIX_KEY_PREFIX,
};
use crate::{AppError, AppErrorKind};

#[test]
fn prefix_job_key_uses_the_canonical_effective_prefix() {
    let root = tempfile::tempdir().unwrap();
    let pfx = root.path().join("pfx");
    std::fs::create_dir(&pfx).unwrap();
    let canonical_pfx = std::fs::canonicalize(&pfx).unwrap();

    let (canonical, key) = prefix_job_key(root.path().to_str().unwrap()).unwrap();
    assert_eq!(canonical, canonical_pfx.to_string_lossy());
    assert_eq!(
        key.as_str(),
        format!("{PREFIX_KEY_PREFIX}{}", canonical_pfx.display())
    );
    // The raw prefix and its `pfx` directory share one key.
    let (_, same) = prefix_job_key(pfx.to_str().unwrap()).unwrap();
    assert_eq!(same, key);
    assert_eq!(prefix_job_key_for_path(&canonical_pfx), key);
}

#[test]
fn prefix_job_key_rejects_a_missing_prefix() {
    let root = tempfile::tempdir().unwrap();
    let missing = root.path().join("missing");
    let error = prefix_job_key(missing.to_str().unwrap()).unwrap_err();
    assert_eq!(
        error.kind(),
        AppErrorKind::Validation {
            field: "prefix_path"
        }
    );
}

#[test]
fn conflict_as_rewrites_only_conflicts() {
    let conflict = AppError::new(AppErrorKind::Conflict, "a job is already running");
    let rewritten = conflict_as(conflict, MSG_PREFIX_BUSY);
    assert_eq!(rewritten.kind(), AppErrorKind::Conflict);
    assert_eq!(rewritten.message(), MSG_PREFIX_BUSY);

    let other = AppError::new(AppErrorKind::Unavailable, "no runtime");
    let kept = conflict_as(other.clone(), MSG_PREFIX_BUSY);
    assert_eq!(kept, other);
}
