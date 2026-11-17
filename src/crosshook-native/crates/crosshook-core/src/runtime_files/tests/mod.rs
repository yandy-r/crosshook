//! Tests for the `runtime_files` module.

mod entry;
mod location;
mod open;
mod prune;
mod secure_dir;

use std::io;

use nix::errno::Errno;

use super::RuntimeDirError;

#[test]
fn display_texts_never_contain_a_path_separator() {
    let variants = [
        RuntimeDirError::NoUsableDir,
        RuntimeDirError::NotADirectory,
        RuntimeDirError::ForeignOwner,
        RuntimeDirError::UnsafeFile,
        RuntimeDirError::NamesExhausted,
        RuntimeDirError::Io(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "/run/user/1000/secret",
        )),
    ];
    for variant in variants {
        // The fixed text "I/O" is the only legitimate slash.
        let text = variant.to_string().replace("I/O", "IO");
        assert!(!text.contains('/'), "{variant:?}");
    }
}

#[test]
fn io_display_uses_only_the_error_kind() {
    let error = RuntimeDirError::Io(io::Error::new(io::ErrorKind::NotFound, "/tmp/x"));
    assert_eq!(error.to_string(), "log file I/O failed: entity not found");
}

#[test]
fn errno_eloop_and_enotdir_map_to_not_a_directory() {
    assert!(matches!(
        RuntimeDirError::from(Errno::ELOOP),
        RuntimeDirError::NotADirectory
    ));
    assert!(matches!(
        RuntimeDirError::from(Errno::ENOTDIR),
        RuntimeDirError::NotADirectory
    ));
    assert!(matches!(
        RuntimeDirError::from(Errno::EACCES),
        RuntimeDirError::Io(_)
    ));
}
