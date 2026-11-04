//! Cross-binary single-instance guard: a runtime-dir `flock` (Phase A step
//! 0; contention is `BootstrapError::AlreadyRunning`) plus the session-bus
//! name `dev.crosshook.CrossHook` serving `org.freedesktop.Application`
//! (Phase B). A second launch calls [`activate_running_instance`]; the
//! running UI raises its window through its [`ActivationHandler`]. Lock files
//! are empty, runtime-only and released by the kernel on process death.

use std::ffi::OsStr;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

mod bus;
mod file;
mod guard;
mod relay;

use file::{attempt, open_all, watch_closes, Attempt, OpenedLock};

pub(crate) use bus::claim_bus_name;
#[cfg(feature = "test-seams")]
#[doc(hidden)]
pub use bus::{activate_named, claim_bus_name_at};
pub use bus::{
    activate_running_instance, ActivationError, ActivationHandler, ActivationRequest,
    ActivationStatus, BusClaim, ClaimOutcome,
};
pub use guard::{HandoffError, InstanceGuard};
pub(crate) use relay::ActivationRelay;

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

/// Held exclusive `flock`s on the instance lock files. Dropping it closes the
/// descriptors, which releases the locks; process exit does the same. The
/// descriptors are close-on-exec, so spawned games never inherit them.
#[derive(Debug)]
pub struct InstanceLock {
    _files: Vec<File>,
    paths: InstanceLockPaths,
    covers_shared: bool,
}

/// Fresh opens after a lock file was replaced at its path, per acquire.
const MAX_REOPENS: u32 = 8;

impl InstanceLock {
    /// Takes the lock without blocking.
    pub fn try_acquire(paths: &InstanceLockPaths) -> Result<Self, AcquireError> {
        for _ in 0..MAX_REOPENS {
            let opened = open_all(paths)?;
            match attempt(&opened) {
                Attempt::Locked => return Ok(Self::held(opened, paths)),
                Attempt::Contended => return Err(AcquireError::Contended),
                Attempt::Failed(error) => return Err(error),
                Attempt::Stale => {}
            }
        }
        Err(stale_error())
    }

    /// Takes the lock, waiting up to `timeout` for the holder to close it.
    ///
    /// Tries once; on contention it watches every lock file it actually
    /// opened for `IN_CLOSE` and loops try → `poll` with a deadline (no
    /// sleeps, no threads). A file replaced at its path is reopened within
    /// the same deadline. It fails closed: once the lock is known to be
    /// held, any later failure (watch, wait, lock error) is
    /// [`AcquireError::Contended`], never `Unavailable`, so a handed-off
    /// launch cannot start beside a live holder.
    pub fn acquire_with_wait(
        paths: &InstanceLockPaths,
        timeout: Duration,
    ) -> Result<Self, AcquireError> {
        let deadline = Instant::now().checked_add(timeout);
        let mut contended = false;
        let mut reopens = 0;
        let mut opened = open_all(paths)?;
        let mut watch = None;
        loop {
            match attempt(&opened) {
                Attempt::Locked => return Ok(Self::held(opened, paths)),
                Attempt::Contended => contended = true,
                Attempt::Failed(_) if contended => {
                    tracing::warn!("instance lock retry failed; treating the lock as held");
                    return Err(AcquireError::Contended);
                }
                Attempt::Failed(error) => return Err(error),
                Attempt::Stale => {
                    reopens += 1;
                    if reopens > MAX_REOPENS {
                        return Err(if contended {
                            AcquireError::Contended
                        } else {
                            stale_error()
                        });
                    }
                    opened = match open_all(paths) {
                        Ok(fresh) => fresh,
                        Err(_) if contended => return Err(AcquireError::Contended),
                        Err(error) => return Err(error),
                    };
                    watch = None;
                    continue;
                }
            }
            let remaining = match deadline {
                Some(deadline) => deadline.saturating_duration_since(Instant::now()),
                None => Duration::MAX,
            };
            if remaining.is_zero() {
                return Err(AcquireError::Contended);
            }
            if watch.is_none() {
                // Armed after a contended try; the next try covers a close
                // that happened in between.
                match watch_closes(&opened) {
                    Ok(inotify) => watch = Some(inotify),
                    Err(error) => {
                        tracing::warn!(%error, "instance lock wait unavailable; treating the lock as held");
                        return Err(AcquireError::Contended);
                    }
                }
                continue;
            }
            if let Some(inotify) = &watch {
                if let Err(error) = file::wait_for_close(inotify, remaining) {
                    tracing::warn!(%error, "instance lock wait failed; treating the lock as held");
                    return Err(AcquireError::Contended);
                }
            }
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

    /// Whether this lock also covers the Flatpak app's lock file, so a
    /// Flatpak CrossHook is excluded without the session bus. False in
    /// native mode when the shared directory was unusable.
    pub fn covers_flatpak(&self) -> bool {
        self.covers_shared
    }

    fn held(opened: Vec<OpenedLock>, paths: &InstanceLockPaths) -> Self {
        let covers_shared = paths.shared.is_none() || opened.len() > 1;
        Self {
            _files: opened.into_iter().map(|lock| lock.file).collect(),
            paths: paths.clone(),
            covers_shared,
        }
    }
}

fn stale_error() -> AcquireError {
    file::io_error(io::Error::other("instance lock file keeps being replaced"))
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
