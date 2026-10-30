//! Turns `SIGINT`/`SIGTERM`/`SIGHUP` into a normal app exit.
//!
//! tao never fires `RunEvent::Exit` for a termination signal: without a
//! handler the process dies at once, so the `[ui]` flush and
//! `App::shutdown` never run and supervised process groups (installers,
//! winetricks) outlive CrossHook. Supervised children lead their own process
//! groups, so a signal to CrossHook's group no longer reaches them either.

use tauri::AppHandle;
use tokio::signal::unix::{signal, Signal, SignalKind};

/// Listeners for the termination signals. Installing them replaces the
/// default (terminate) disposition for the rest of the process.
struct Termination {
    interrupt: Signal,
    terminate: Signal,
    hangup: Signal,
}

impl Termination {
    /// Installs the listeners. Must run inside a Tokio runtime.
    fn install() -> std::io::Result<Self> {
        Ok(Self {
            interrupt: signal(SignalKind::interrupt())?,
            terminate: signal(SignalKind::terminate())?,
            hangup: signal(SignalKind::hangup())?,
        })
    }

    /// Resolves with the name of the first signal received.
    async fn recv(&mut self) -> &'static str {
        tokio::select! {
            _ = self.interrupt.recv() => "SIGINT",
            _ = self.terminate.recv() => "SIGTERM",
            _ = self.hangup.recv() => "SIGHUP",
        }
    }
}

/// Spawns the listener: the first termination signal requests a normal exit
/// (`ExitRequested`, then `Exit`, which flushes settings and shuts the app
/// down within its bound). Later signals are ignored while that runs.
pub(crate) fn exit_on_termination(handle: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut termination = match Termination::install() {
            Ok(termination) => termination,
            Err(error) => {
                tracing::warn!(%error, "termination signal handler not installed");
                return;
            }
        };
        let name = termination.recv().await;
        tracing::info!(signal = name, "termination signal received; exiting");
        handle.exit(0);
        // Keep the listeners alive: dropping them would not restore the
        // default disposition, and a second signal must not end the bounded
        // shutdown early.
        std::future::pending::<()>().await;
    });
}

#[cfg(test)]
mod tests {
    use nix::sys::signal::{raise, Signal as NixSignal};

    use super::Termination;

    #[tokio::test]
    async fn termination_signals_are_caught_instead_of_killing_the_process() {
        let mut termination = Termination::install().expect("install signal listeners");
        // Without the listeners these would terminate the test process.
        for (signal, name) in [
            (NixSignal::SIGHUP, "SIGHUP"),
            (NixSignal::SIGTERM, "SIGTERM"),
            (NixSignal::SIGINT, "SIGINT"),
        ] {
            raise(signal).expect("raise signal");
            assert_eq!(termination.recv().await, name);
        }
    }
}
