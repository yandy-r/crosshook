//! Cross-binary single-instance guard: a runtime-dir `flock` (Phase A step
//! 0; contention is `BootstrapError::AlreadyRunning`) plus the session-bus
//! name `dev.crosshook.CrossHook` serving `org.freedesktop.Application`
//! (Phase B). A second launch calls [`activate_running_instance`]; the
//! running UI raises its window through its [`ActivationHandler`]. Lock files
//! are empty, runtime-only and released by the kernel on process death.

use std::ffi::OsStr;
use std::fs::{DirBuilder, File, OpenOptions, TryLockError};
use std::io;
use std::os::fd::AsFd;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use nix::errno::Errno;
use nix::fcntl::OFlag;
use nix::poll::{PollFd, PollFlags, PollTimeout};
use nix::sys::inotify::{AddWatchFlags, InitFlags, Inotify};
use nix::sys::stat::Mode;

mod bus;
mod guard;

#[cfg(feature = "test-seams")]
#[doc(hidden)]
pub use bus::{activate_named, claim_bus_name_at};
pub use bus::{
    activate_running_instance, ActivationError, ActivationHandler, ActivationRequest,
    ActivationStatus, BusClaim, ClaimOutcome,
};
pub(crate) use bus::{claim_bus_name, ActivationRelay};
pub use guard::{HandoffError, InstanceGuard};

/// Application id: the session-bus name and the Flatpak runtime directory.
pub const APP_ID: &str = "dev.crosshook.CrossHook";
/// Object path exporting `org.freedesktop.Application`.
pub const OBJECT_PATH: &str = "/dev/crosshook/CrossHook";
/// Set to `1` on a handed-off launch; Phase A then waits for the lock.
pub const HANDOFF_ENV: &str = "CROSSHOOK_INSTANCE_HANDOFF";
/// How long a handed-off launch waits for the previous owner's lock.
///
/// The spec says 5 s, but the handing-off process keeps its lock until it
/// exits (so it never races the new process on the stores), and the Classic
/// exit path can take about 8 s (the `[ui]` flush plus
/// `App::DEFAULT_SHUTDOWN_TIMEOUT`). 10 s covers that worst case.
pub const HANDOFF_WAIT: Duration = Duration::from_secs(10);
/// Budget for a second instance to activate the running one.
pub const ACTIVATE_TIMEOUT: Duration = Duration::from_secs(2);

const LOCK_FILE: &str = "instance.lock";
const NATIVE_DIR: &str = "crosshook";

#[derive(Debug, Clone, PartialEq, Eq)]
struct LockLocation {
    dir: PathBuf,
    file: PathBuf,
    create_parents: bool,
}

impl LockLocation {
    fn new(dir: PathBuf, create_parents: bool) -> Self {
        let file = dir.join(LOCK_FILE);
        Self {
            dir,
            file,
            create_parents,
        }
    }

    /// `<runtime>/app/dev.crosshook.CrossHook`: the Flatpak per-app runtime
    /// directory, which a native run sees at the same host path.
    fn shared(runtime: &Path) -> Self {
        Self::new(runtime.join("app").join(APP_ID), true)
    }
}

/// Where the lock lives. Only constructible from a runtime directory.
///
/// Native runs lock two files: `<runtime>/crosshook/instance.lock` and the
/// Flatpak app's `<runtime>/app/dev.crosshook.CrossHook/instance.lock`, so a
/// native and a Flatpak CrossHook exclude each other before any migration.
/// The shared lock is best-effort in native mode (an unusable directory only
/// warns); the Flatpak locks its app directory alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceLockPaths {
    primary: LockLocation,
    shared: Option<LockLocation>,
}

impl InstanceLockPaths {
    /// Resolves the lock location under `$XDG_RUNTIME_DIR`. An unset, empty
    /// or relative runtime directory is [`LockUnavailable::RuntimeDirUnavailable`].
    pub fn resolve(runtime_dir: Option<&OsStr>, flatpak: bool) -> Result<Self, LockUnavailable> {
        let root = runtime_dir
            .map(Path::new)
            .filter(|path| path.is_absolute())
            .ok_or(LockUnavailable::RuntimeDirUnavailable)?;
        Ok(if flatpak {
            Self {
                primary: LockLocation::shared(root),
                shared: None,
            }
        } else {
            Self::in_dir(root)
        })
    }

    /// Native layout under `dir`; equal to `resolve(Some(dir), false)`. Used
    /// by tests and the ordering harness.
    pub fn in_dir(dir: impl Into<PathBuf>) -> Self {
        let dir = dir.into();
        Self {
            primary: LockLocation::new(dir.join(NATIVE_DIR), false),
            shared: Some(LockLocation::shared(&dir)),
        }
    }

    /// The primary lock file path.
    pub fn file(&self) -> &Path {
        &self.primary.file
    }

    /// The private directory holding the primary lock file.
    pub fn dir(&self) -> &Path {
        &self.primary.dir
    }

    /// The Flatpak-shared lock file a native run also takes, if any.
    pub fn shared_file(&self) -> Option<&Path> {
        self.shared.as_ref().map(|location| location.file.as_path())
    }
}

/// Why the lock could not be used; startup continues on the bus name alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockUnavailable {
    /// `$XDG_RUNTIME_DIR` is unset, empty or relative.
    RuntimeDirUnavailable,
    /// The lock directory is a symlink, foreign-owned or group/other writable.
    RuntimeDirUnsafe,
    /// Creating, opening or locking failed.
    Io,
}

impl LockUnavailable {
    /// Static reason recorded as the degraded step outcome.
    pub fn reason(self) -> &'static str {
        match self {
            Self::RuntimeDirUnavailable => "runtime dir unavailable",
            Self::RuntimeDirUnsafe => "runtime dir unsafe",
            Self::Io => "instance lock unavailable",
        }
    }
}

/// Failure to take the lock.
#[derive(Debug)]
pub enum AcquireError {
    /// Another process holds the lock.
    Contended,
    /// The lock cannot be used at all, with the underlying error when known.
    Unavailable(LockUnavailable, Option<io::Error>),
}

fn io_error(error: impl Into<io::Error>) -> AcquireError {
    AcquireError::Unavailable(LockUnavailable::Io, Some(error.into()))
}

fn unsafe_dir(error: Option<io::Error>) -> AcquireError {
    AcquireError::Unavailable(LockUnavailable::RuntimeDirUnsafe, error)
}

/// Held exclusive `flock`s on the instance lock files. Dropping it closes the
/// descriptors, which releases the locks; process exit does the same. The
/// descriptors are close-on-exec, so spawned games never inherit them.
#[derive(Debug)]
pub struct InstanceLock {
    _files: Vec<File>,
    paths: InstanceLockPaths,
}

impl InstanceLock {
    /// Takes the lock without blocking.
    pub fn try_acquire(paths: &InstanceLockPaths) -> Result<Self, AcquireError> {
        let files = open_all(paths)?;
        lock_all(&files)?;
        Ok(Self::held(files, paths))
    }

    /// Takes the lock, waiting up to `timeout` for the holder to close it: an
    /// inotify `IN_CLOSE` watch on every lock file set before the first try,
    /// then `poll` with a deadline (no sleeps, no threads).
    pub fn acquire_with_wait(
        paths: &InstanceLockPaths,
        timeout: Duration,
    ) -> Result<Self, AcquireError> {
        let deadline = Instant::now().checked_add(timeout);
        let files = open_all(paths)?;
        let inotify =
            Inotify::init(InitFlags::IN_CLOEXEC | InitFlags::IN_NONBLOCK).map_err(io_error)?;
        for location in [Some(&paths.primary), paths.shared.as_ref()]
            .into_iter()
            .flatten()
        {
            inotify
                .add_watch(
                    &location.file,
                    AddWatchFlags::IN_CLOSE | AddWatchFlags::IN_DONT_FOLLOW,
                )
                .map_err(io_error)?;
        }
        loop {
            match lock_all(&files) {
                Ok(()) => return Ok(Self::held(files, paths)),
                Err(AcquireError::Contended) => {}
                Err(error) => return Err(error),
            }
            let remaining = match deadline {
                Some(deadline) => deadline.saturating_duration_since(Instant::now()),
                None => Duration::MAX,
            };
            if remaining.is_zero() {
                return Err(AcquireError::Contended);
            }
            wait_for_close(&inotify, remaining)?;
        }
    }

    /// Spawns `cmd` with `CROSSHOOK_INSTANCE_HANDOFF=1` while the lock stays
    /// held. The new process waits up to [`HANDOFF_WAIT`] for it; the kernel
    /// releases it when this process exits, so the caller must exit right
    /// after `Ok` (and must not touch the stores again).
    pub fn spawn_handoff(&self, mut cmd: Command) -> io::Result<Child> {
        cmd.env(HANDOFF_ENV, "1").spawn()
    }

    /// Where this lock lives.
    pub fn paths(&self) -> &InstanceLockPaths {
        &self.paths
    }

    fn held(files: Vec<File>, paths: &InstanceLockPaths) -> Self {
        Self {
            _files: files,
            paths: paths.clone(),
        }
    }
}

/// Opens every lock file: the primary must open; the native run's shared
/// file is skipped with a warning when its directory is unusable.
fn open_all(paths: &InstanceLockPaths) -> Result<Vec<File>, AcquireError> {
    let mut files = vec![open_lock_file(&paths.primary)?];
    if let Some(shared) = &paths.shared {
        match open_lock_file(shared) {
            Ok(file) => files.push(file),
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

/// Locks every file or none: on contention or error the locks taken in this
/// attempt are released again.
fn lock_all(files: &[File]) -> Result<(), AcquireError> {
    for (index, file) in files.iter().enumerate() {
        let error = match file.try_lock() {
            Ok(()) => continue,
            Err(TryLockError::WouldBlock) => AcquireError::Contended,
            Err(TryLockError::Error(error)) => io_error(error),
        };
        for taken in files.iter().take(index) {
            if let Err(unlock) = taken.unlock() {
                tracing::warn!(error = %unlock, "could not release a partial instance lock");
            }
        }
        return Err(error);
    }
    Ok(())
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
        LOCK_FILE,
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

/// Waits for an `IN_CLOSE` on a lock file or `remaining`, then drains events.
fn wait_for_close(inotify: &Inotify, remaining: Duration) -> Result<(), AcquireError> {
    let millis = remaining.as_nanos().div_ceil(1_000_000);
    let timeout = PollTimeout::try_from(millis).unwrap_or(PollTimeout::MAX);
    let mut fds = [PollFd::new(inotify.as_fd(), PollFlags::POLLIN)];
    match nix::poll::poll(&mut fds, timeout) {
        Ok(_) | Err(Errno::EINTR) => {}
        Err(error) => return Err(io_error(error)),
    }
    loop {
        match inotify.read_events() {
            Ok(events) if !events.is_empty() => {}
            Ok(_) | Err(Errno::EAGAIN) => return Ok(()),
            Err(error) => return Err(io_error(error)),
        }
    }
}

/// Outcome of the Phase A lock attempt (host seam).
#[derive(Debug)]
pub enum InstanceLockAttempt {
    /// This process now holds the lock.
    Acquired(InstanceLock),
    /// Another instance holds it.
    Contended,
    /// The lock cannot be used; the static reason is recorded as degraded.
    Unavailable(&'static str),
}

/// What Phase A carries forward about the lock.
#[derive(Debug)]
pub enum InstanceLockState {
    /// The lock is held for the process lifetime.
    Held(InstanceLock),
    /// Running without the lock, for the given reason.
    Unavailable(&'static str),
}

#[cfg(test)]
mod tests;
