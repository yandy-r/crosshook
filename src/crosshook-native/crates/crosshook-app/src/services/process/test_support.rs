//! Test helper shared by the process services: a descendant of a fake
//! runner that leaves the job's process group (`setsid`, like wineserver and
//! Wine GUI processes) but keeps the prefix in its environment, so only the
//! prefix sweep can reach it.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;

/// Upper bound for the escapee to die after a cancel or stop; only reached
/// when the sweep is broken (the test then fails instead of hanging).
const DEATH_BOUND: Duration = Duration::from_secs(30);

/// Where an escapee's rendezvous files live (outside the prefix, which a
/// run may delete).
pub(crate) struct EscapeeFiles {
    alive: PathBuf,
    hold: PathBuf,
    pid: PathBuf,
}

impl EscapeeFiles {
    pub(crate) fn in_dir(dir: &Path) -> Self {
        Self {
            alive: dir.join("escapee-alive"),
            hold: dir.join("escapee-hold"),
            pid: dir.join("escapee.pid"),
        }
    }

    /// Shell lines for the fake runner that start the escapee in the
    /// background. It inherits the runner's environment (`WINEPREFIX`),
    /// records its pid and holds `alive` open for writing until it dies.
    pub(crate) fn script(&self) -> String {
        format!(
            "mkfifo '{alive}' '{hold}'\n\
             setsid sh -c 'echo $$ > \"$0\"; exec 3>\"$1\"; read x < \"$2\"' '{pid}' '{alive}' '{hold}' &",
            alive = self.alive.display(),
            hold = self.hold.display(),
            pid = self.pid.display(),
        )
    }

    /// Rendezvous with the escapee's writer (blocks until it opened `alive`):
    /// it is alive, outside the group, and its pid is recorded from here on.
    pub(crate) async fn attach(&self) -> Escapee {
        let alive_path = self.alive.clone();
        let alive = tokio::task::spawn_blocking(move || std::fs::File::open(alive_path))
            .await
            .unwrap()
            .unwrap();
        let pid = std::fs::read_to_string(&self.pid)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        Escapee {
            pid,
            alive: Some(alive),
            dead: false,
        }
    }
}

/// A live escapee (see [`EscapeeFiles::attach`]). Dropping it before its
/// death was observed (a failed assertion unwinding) `SIGKILL`s it, so a
/// failing test leaves nothing running.
pub(crate) struct Escapee {
    pid: i32,
    alive: Option<std::fs::File>,
    dead: bool,
}

impl Drop for Escapee {
    fn drop(&mut self) {
        if !self.dead {
            let _ = kill(Pid::from_raw(self.pid), Signal::SIGKILL);
        }
    }
}

impl Escapee {
    /// Whether the escapee died (EOF on `alive`) within the bound; a
    /// survivor is killed on drop so the test can finish.
    pub(crate) async fn died(mut self) -> bool {
        let Some(mut alive) = self.alive.take() else {
            return self.dead;
        };
        let eof = tokio::time::timeout(
            DEATH_BOUND,
            tokio::task::spawn_blocking(move || {
                let mut rest = Vec::new();
                alive.read_to_end(&mut rest).map(|_| rest)
            }),
        )
        .await;
        // A timeout leaves `dead` unset: `Drop` kills the survivor, which
        // also unblocks the reader.
        self.dead = matches!(eof, Ok(Ok(Ok(rest))) if rest.is_empty());
        self.dead
    }
}
