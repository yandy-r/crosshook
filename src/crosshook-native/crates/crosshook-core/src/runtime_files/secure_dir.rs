//! Verified private directory handle.
//!
//! The log directory is opened component by component relative to a verified
//! root fd with `O_NOFOLLOW`, so a symlink planted at any component is refused
//! instead of followed. Every component must be owned by the expected user and
//! has its group/other permission bits stripped through its own fd.

use std::fs::DirBuilder;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};

use nix::errno::Errno;
use nix::fcntl::{open, openat, OFlag};
use nix::sys::stat::{fchmod, fstat, mkdirat, Mode};
use nix::unistd::geteuid;

use super::{CandidateRoot, LogDirCandidate, RuntimeDirError};

/// Permission bits of a private directory (owner only).
const PRIVATE_DIR_MODE: u32 = 0o700;
/// Group and other write bits; a runtime root carrying either is not trusted.
const GROUP_OTHER_WRITE: u32 = 0o022;
/// Other write bit. A state root (a pre-existing home directory such as
/// `~/.local/state`) may be group-writable under a user-private-group umask;
/// privacy is enforced on the owner-only `crosshook/logs` components below it.
const OTHER_WRITE: u32 = 0o002;

/// A verified private directory, held open by file descriptor.
#[derive(Debug)]
pub struct PrivateDir {
    fd: OwnedFd,
    path: PathBuf,
}

impl PrivateDir {
    /// The directory path, for display and path-based fallbacks only.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl AsFd for PrivateDir {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

/// Opens (creating missing components) the private directory of `candidate`.
///
/// The root must be a directory owned by the current user that is not group or
/// other writable. A `Runtime` root is never created; a `State` root is.
pub fn open_private_dir(candidate: &LogDirCandidate) -> Result<PrivateDir, RuntimeDirError> {
    open_private_dir_owned_by(candidate, geteuid().as_raw())
}

/// [`open_private_dir`] with an explicit expected owner (test seam).
pub(crate) fn open_private_dir_owned_by(
    candidate: &LogDirCandidate,
    owner: u32,
) -> Result<PrivateDir, RuntimeDirError> {
    open_private_dir_with_owners(candidate, owner, owner)
}

/// Like the seam above but with separate expected owners for the root and for
/// the components below it, so a foreign-owned component is testable.
pub(crate) fn open_private_dir_with_owners(
    candidate: &LogDirCandidate,
    root_owner: u32,
    component_owner: u32,
) -> Result<PrivateDir, RuntimeDirError> {
    let mut fd = open_root(candidate, root_owner)?;
    for component in &candidate.relative {
        match mkdirat(&fd, *component, Mode::from_bits_truncate(PRIVATE_DIR_MODE)) {
            Ok(()) | Err(Errno::EEXIST) => {}
            Err(errno) => return Err(errno.into()),
        }
        let next = openat(
            &fd,
            *component,
            dir_flags() | OFlag::O_NOFOLLOW,
            Mode::empty(),
        )?;
        let stat = fstat(&next)?;
        if stat.st_uid != component_owner {
            return Err(RuntimeDirError::ForeignOwner);
        }
        fchmod(
            &next,
            Mode::from_bits_truncate(stat.st_mode & PRIVATE_DIR_MODE),
        )?;
        fd = next;
    }
    Ok(PrivateDir {
        fd,
        path: candidate.path(),
    })
}

fn dir_flags() -> OFlag {
    OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_CLOEXEC
}

/// Opens and checks the root. `O_NOFOLLOW` is deliberately absent: the Flatpak
/// `app` entry under the runtime directory is a symlink.
fn open_root(candidate: &LogDirCandidate, owner: u32) -> Result<OwnedFd, RuntimeDirError> {
    if candidate.kind == CandidateRoot::State {
        DirBuilder::new()
            .recursive(true)
            .mode(PRIVATE_DIR_MODE)
            .create(&candidate.root)?;
    }
    let fd = open(&candidate.root, dir_flags(), Mode::empty())
        .map_err(|_| RuntimeDirError::NoUsableDir)?;
    let stat = fstat(&fd).map_err(|_| RuntimeDirError::NoUsableDir)?;
    let forbidden = match candidate.kind {
        CandidateRoot::State => OTHER_WRITE,
        _ => GROUP_OTHER_WRITE,
    };
    if stat.st_uid != owner || stat.st_mode & forbidden != 0 {
        return Err(RuntimeDirError::NoUsableDir);
    }
    Ok(fd)
}
