//! Phase B single-instance step: own the session-bus name before any store.

use std::sync::Arc;

use tokio::runtime::{Handle, RuntimeFlavor};

use super::{BootstrapError, BootstrapStep, InstanceBus, StepLog, StepOutcome};
use crate::instance_lock::{claim_bus_name, ActivationRelay, BusClaim, ClaimOutcome};
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

/// Whether nothing guards this process: Phase A has no lock and the bus step
/// degraded. A skipped bus (explicit shell choice) counts as guarded.
pub(super) fn unguarded(holds_lock: bool, steps: &StepLog) -> bool {
    !holds_lock
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
    fn skip_records_reason_without_touching_the_bus() {
        let app = App::new().expect("owned runtime");
        let mut steps = StepLog::new();
        let claim = instance_bus_step(
            &app,
            InstanceBus::Skip {
                reason: "unit test",
            },
            Arc::default(),
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
        let claim = instance_bus_step(&app, InstanceBus::Enabled, Arc::default(), &mut steps)
            .expect("degrades, never fails");
        assert!(claim.is_none());
        assert_eq!(
            steps.records()[0].outcome,
            StepOutcome::Degraded("instance bus needs a non-runtime caller")
        );
    }
}
