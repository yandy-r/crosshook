//! Tests for the shared process plumbing.

mod child;
mod owned;
mod owner;
mod run;

use super::{
    conflict_as, prefix_job_key, prefix_job_key_for_path, remove_unused_log, MSG_PREFIX_BUSY,
    PREFIX_KEY_PREFIX,
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
fn a_fresh_root_is_keyed_on_the_pfx_proton_will_create() {
    let root = tempfile::tempdir().unwrap();
    let canonical_root = std::fs::canonicalize(root.path()).unwrap();

    // No `pfx` yet: the effective prefix is the root, the key its `pfx`.
    let (canonical, fresh) = prefix_job_key(root.path().to_str().unwrap()).unwrap();
    assert_eq!(canonical, canonical_root.to_string_lossy());
    assert_eq!(
        fresh.as_str(),
        format!(
            "{PREFIX_KEY_PREFIX}{}",
            canonical_root.join("pfx").display()
        )
    );

    // Once Proton created `pfx`, every later job lands on the same key.
    std::fs::create_dir(canonical_root.join("pfx")).unwrap();
    let (_, later) = prefix_job_key(root.path().to_str().unwrap()).unwrap();
    assert_eq!(later, fresh);
    assert_eq!(prefix_job_key_for_path(&canonical_root), fresh);
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

#[test]
fn remove_unused_log_deletes_the_file_and_tolerates_a_missing_one() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("unused.log");
    std::fs::write(&log, b"").unwrap();
    remove_unused_log(&log);
    assert!(!log.exists());
    // Already gone: only logged.
    remove_unused_log(&log);
}
