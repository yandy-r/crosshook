//! Process-spawn seam for the launch service.
//!
//! The spawner receives the command already built by `crosshook-core`. It has
//! no kill, wait or process-group method: the service keeps a plain
//! `child.wait()`, and the only kill paths are the core gamescope watchdog on
//! the session's own cancel channel.

use std::io;

use tokio::process::{Child, Command};

/// Starts the process described by an already-built command.
pub trait ProcessSpawner: Send + Sync + 'static {
    /// Spawns `command`.
    ///
    /// # Errors
    /// Returns the I/O error of the failed spawn.
    fn spawn(&self, command: &mut Command) -> io::Result<Child>;
}

/// Production spawner: runs `command.spawn()`.
#[derive(Debug, Clone, Copy, Default)]
pub struct TokioSpawner;

impl ProcessSpawner for TokioSpawner {
    fn spawn(&self, command: &mut Command) -> io::Result<Child> {
        command.spawn()
    }
}
