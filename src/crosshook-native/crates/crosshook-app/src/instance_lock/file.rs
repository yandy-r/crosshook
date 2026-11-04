//! Lock-file mechanics: safe open, all-or-none locking with an identity
//! check, and the inotify close wait.

use std::fs::{DirBuilder, File, OpenOptions, TryLockError};
use std::io;
use std::os::fd::AsFd;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::Duration;

use nix::errno::Errno;
use nix::fcntl::OFlag;
use nix::poll::{PollFd, PollFlags, PollTimeout};
use nix::sys::inotify::{AddWatchFlags, InitFlags, Inotify};
use nix::sys::stat::Mode;

use super::{AcquireError, InstanceLockPaths, LockLocation, LockUnavailable, LOCK_FILE};

pub(super) fn io_error(error: impl Into<io::Error>) -> AcquireError {
    AcquireError::Unavailable(LockUnavailable::Io, Some(error.into()))
}

fn unsafe_dir(error: Option<io::Error>) -> AcquireError {
    AcquireError::Unavailable(LockUnavailable::RuntimeDirUnsafe, error)
}

/// One opened lock file and the path it was opened at.
pub(super) struct OpenedLock {
    pub(super) file: File,
    path: PathBuf,
}

/// Result of one locking attempt over every opened file.
pub(super) enum Attempt {
    /// Every file is locked and still named by its path.
    Locked,
    /// Another process holds a lock.
    Contended,
    /// A file was replaced at its path after it was opened; nothing is held.
    Stale,
    /// Locking failed; nothing is held.
    Failed(AcquireError),
}

/// Opens every lock file: the primary must open; the native run's shared
/// file is skipped with a warning when its directory is unusable.
pub(super) fn open_all(paths: &InstanceLockPaths) -> Result<Vec<OpenedLock>, AcquireError> {
    let opened = |location: &LockLocation, file| OpenedLock {
        file,
        path: location.file.clone(),
    };
    let mut files = vec![opened(&paths.primary, open_lock_file(&paths.primary)?)];
    if let Some(shared) = &paths.shared {
        match open_lock_file(shared) {
            Ok(file) => files.push(opened(shared, file)),
            Err(AcquireError::Unavailable(unavailable, error)) => {
                let detail =
                    error.map(|error| crate::presenter::sanitize_message(&error.to_string()));
                tracing::warn!(
                    reason = unavailable.reason(),
                    ?detail,
                    "shared instance lock skipped"
                );
                eprintln!(
                    "CrossHook: shared instance lock skipped ({}); a Flatpak CrossHook is only excluded by the session bus",
                    unavailable.reason()
                );
            }
            Err(error @ AcquireError::Contended) => return Err(error),
        }
    }
    Ok(files)
}

/// Locks every file or none, then checks that each locked descriptor is
/// still the file at its path (device and inode). A file replaced between
/// open and lock would otherwise be a lock nobody else can see.
///
/// This cannot stop a same-user process from unlinking the file while an
/// instance holds it: the replacement then matches its path. `flock` gives
/// no protection against that, and the bus name still excludes the second
/// instance in Phase B.
pub(super) fn attempt(files: &[OpenedLock]) -> Attempt {
    for (index, lock) in files.iter().enumerate() {
        let outcome = match lock.file.try_lock() {
            Ok(()) => continue,
            Err(TryLockError::WouldBlock) => Attempt::Contended,
            Err(TryLockError::Error(error)) => Attempt::Failed(io_error(error)),
        };
        unlock(&files[..index]);
        return outcome;
    }
    if files.iter().all(is_current) {
        Attempt::Locked
    } else {
        unlock(files);
        Attempt::Stale
    }
}

fn unlock(files: &[OpenedLock]) {
    for lock in files {
        if let Err(error) = lock.file.unlock() {
            tracing::warn!(%error, "could not release a partial instance lock");
        }
    }
}

/// Whether the descriptor is still the file named by its path.
fn is_current(lock: &OpenedLock) -> bool {
    let (Ok(held), Ok(named)) = (lock.file.metadata(), std::fs::symlink_metadata(&lock.path))
    else {
        return false;
    };
    held.dev() == named.dev() && held.ino() == named.ino()
}

/// Watches every opened lock file for `IN_CLOSE`.
pub(super) fn watch_closes(opened: &[OpenedLock]) -> nix::Result<Inotify> {
    let inotify = Inotify::init(InitFlags::IN_CLOEXEC | InitFlags::IN_NONBLOCK)?;
    for lock in opened {
        inotify.add_watch(
            &lock.path,
            AddWatchFlags::IN_CLOSE | AddWatchFlags::IN_DONT_FOLLOW,
        )?;
    }
    Ok(inotify)
}

/// Waits for an `IN_CLOSE` on a lock file or `remaining`, then drains events.
pub(super) fn wait_for_close(inotify: &Inotify, remaining: Duration) -> nix::Result<()> {
    let millis = remaining.as_nanos().div_ceil(1_000_000);
    let timeout = PollTimeout::try_from(millis).unwrap_or(PollTimeout::MAX);
    let mut fds = [PollFd::new(inotify.as_fd(), PollFlags::POLLIN)];
    match nix::poll::poll(&mut fds, timeout) {
        Ok(_) | Err(Errno::EINTR) => {}
        Err(error) => return Err(error),
    }
    loop {
        match inotify.read_events() {
            Ok(events) if !events.is_empty() => {}
            Ok(_) | Err(Errno::EAGAIN) => return Ok(()),
            Err(error) => return Err(error),
        }
    }
}

/// Opens (creating `0700`) the lock dir; rejects symlinks, foreign owners and group/other write.
fn open_private_dir(location: &LockLocation) -> Result<File, AcquireError> {
    match DirBuilder::new()
        .mode(0o700)
        .recursive(location.create_parents)
        .create(&location.dir)
    {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(io_error(error)),
    }
    let dir = OpenOptions::new()
        .read(true)
        .custom_flags((OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW).bits())
        .open(&location.dir)
        .map_err(
            |error| match Errno::from_raw(error.raw_os_error().unwrap_or(0)) {
                Errno::ELOOP | Errno::ENOTDIR => unsafe_dir(Some(error)),
                _ => io_error(error),
            },
        )?;
    let meta = dir.metadata().map_err(io_error)?;
    let private =
        meta.is_dir() && meta.uid() == nix::unistd::geteuid().as_raw() && meta.mode() & 0o022 == 0;
    if !private {
        return Err(unsafe_dir(None));
    }
    Ok(dir)
}

fn open_lock_file(location: &LockLocation) -> Result<File, AcquireError> {
    let dir = open_private_dir(location)?;
    let fd = nix::fcntl::openat(
        dir.as_fd(),
        Path::new(LOCK_FILE),
        OFlag::O_RDWR | OFlag::O_CREAT | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::S_IRUSR | Mode::S_IWUSR,
    )
    .map_err(io_error)?;
    let file = File::from(fd);
    if !file.metadata().map_err(io_error)?.file_type().is_file() {
        return Err(io_error(io::Error::other(
            "instance lock is not a regular file",
        )));
    }
    Ok(file)
}
