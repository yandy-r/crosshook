//! Phase B single-instance step: own the session-bus name before any store.

use std::sync::Arc;

use tokio::runtime::{Handle, RuntimeFlavor};

use super::{BootstrapError, BootstrapStep, InstanceBus, StepLog, StepOutcome};
use crate::instance_lock::{
    claim_bus_name, ActivationRelay, BusClaim, ClaimOutcome, InstanceLockState,
};
use crate::App;

const BUS_UNAVAILABLE: &str = "session bus unavailable";

/// Claims `dev.crosshook.CrossHook` on the app runtime (bounded by 2 s) and
/// records [`BootstrapStep::InstanceBus`].
///
/// A taken name is [`BootstrapError::AlreadyRunning`] whether or not Phase A
/// holds the lock: it also catches a native run beside the Flatpak, whose
/// lock files differ. A missing bus degrades; `block_on` is never attempted
/// from inside a runtime (it would panic) or on a current-thread runtime
/// (nothing would serve `Activate` afterwards).
pub(super) fn instance_bus_step(
    app: &App,
    choice: InstanceBus,
    relay: Arc<ActivationRelay>,
    steps: &mut StepLog,
) -> Result<Option<BusClaim>, BootstrapError> {
    let (claim, outcome) = match choice {
        InstanceBus::Skip { reason } => (None, StepOutcome::Skipped(reason)),
        InstanceBus::Enabled if Handle::try_current().is_ok() => (
            None,
            StepOutcome::Degraded("instance bus needs a non-runtime caller"),
        ),
        InstanceBus::Enabled
            if app.runtime_handle().runtime_flavor() == RuntimeFlavor::CurrentThread =>
        {
            (
                None,
                StepOutcome::Degraded("instance bus needs a multi-thread runtime"),
            )
        }
        InstanceBus::Enabled => match app.runtime_handle().block_on(claim_bus_name(relay)) {
            ClaimOutcome::Owned(claim) => (Some(claim), StepOutcome::Ran),
            ClaimOutcome::AlreadyOwned => return Err(BootstrapError::AlreadyRunning),
            ClaimOutcome::Unavailable(detail) => {
                eprintln!("CrossHook: {BUS_UNAVAILABLE} ({detail}); single-instance guard limited");
                (None, StepOutcome::Degraded(BUS_UNAVAILABLE))
            }
        },
    };
    steps.push(BootstrapStep::InstanceBus, outcome);
    Ok(claim)
}

/// Whether the held locks exclude both packagings: a native lock whose
/// Flatpak-shared file was skipped does not keep a Flatpak CrossHook out.
pub(super) fn fully_locked(lock: &InstanceLockState) -> bool {
    match lock {
        InstanceLockState::Held(held) => held.covers_flatpak(),
        InstanceLockState::Unavailable(_) => false,
    }
}

/// Whether this process is not fully guarded: the locks do not cover both
/// packagings (no lock, or a native lock whose Flatpak-shared file was
/// skipped) and the bus step degraded. A skipped bus (explicit shell choice)
/// counts as guarded.
pub(super) fn unguarded(fully_locked: bool, steps: &StepLog) -> bool {
    !fully_locked
        && steps.records().iter().any(|record| {
            record.step == BootstrapStep::InstanceBus
                && matches!(record.outcome, StepOutcome::Degraded(_))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log_with_bus(outcome: StepOutcome) -> StepLog {
        let mut steps = StepLog::new();
        steps.push(BootstrapStep::InstanceBus, outcome);
        steps
    }

    #[test]
    fn unguarded_only_without_lock_and_with_degraded_bus() {
        let degraded = log_with_bus(StepOutcome::Degraded(BUS_UNAVAILABLE));
        assert!(unguarded(false, &degraded));
        assert!(!unguarded(true, &degraded));
        assert!(!unguarded(false, &log_with_bus(StepOutcome::Ran)));
        assert!(!unguarded(
            false,
            &log_with_bus(StepOutcome::Skipped("test"))
        ));
    }

    #[test]
    fn partial_native_lock_is_not_fully_locked() {
        use crate::instance_lock::{InstanceLock, InstanceLockPaths};
        use std::os::unix::fs::PermissionsExt;

        let runtime = tempfile::tempdir().unwrap();
        let paths = InstanceLockPaths::in_dir(runtime.path());
        let full = InstanceLock::try_acquire(&paths).unwrap();
        assert!(fully_locked(&InstanceLockState::Held(full)));
        assert!(!fully_locked(&InstanceLockState::Unavailable("test")));

        let other = tempfile::tempdir().unwrap();
        let partial_paths = InstanceLockPaths::in_dir(other.path());
        let shared_dir = partial_paths
            .shared_file()
            .unwrap()
            .parent()
            .unwrap()
            .to_owned();
        std::fs::create_dir_all(&shared_dir).unwrap();
        std::fs::set_permissions(&shared_dir, std::fs::Permissions::from_mode(0o777)).unwrap();
        let partial = InstanceLock::try_acquire(&partial_paths).unwrap();
        let state = InstanceLockState::Held(partial);
        assert!(!fully_locked(&state));
        // Partial lock + degraded bus: the report warns.
        let degraded = log_with_bus(StepOutcome::Degraded(BUS_UNAVAILABLE));
        assert!(unguarded(fully_locked(&state), &degraded));
    }

    #[test]
    fn skip_records_reason_without_touching_the_bus() {
        let app = App::new().expect("owned runtime");
        let mut steps = StepLog::new();
        let claim = instance_bus_step(
            &app,
            InstanceBus::Skip {
                reason: "unit test",
            },
            Arc::new(ActivationRelay::new(app.runtime_handle())),
            &mut steps,
        )
        .expect("skip never fails");
        assert!(claim.is_none());
        assert_eq!(
            steps.records()[0].outcome,
            StepOutcome::Skipped("unit test")
        );
    }

    #[tokio::test]
    async fn enabled_inside_a_runtime_degrades_instead_of_panicking() {
        let app = App::with_handle(Handle::current());
        let mut steps = StepLog::new();
        let relay = Arc::new(ActivationRelay::new(app.runtime_handle()));
        let claim = instance_bus_step(&app, InstanceBus::Enabled, relay, &mut steps)
            .expect("degrades, never fails");
        assert!(claim.is_none());
        assert_eq!(
            steps.records()[0].outcome,
            StepOutcome::Degraded("instance bus needs a non-runtime caller")
        );
    }
}
