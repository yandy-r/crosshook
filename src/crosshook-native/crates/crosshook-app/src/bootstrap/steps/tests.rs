use super::*;

fn pre_runtime_log() -> StepLog {
    let mut log = StepLog::new();
    for step in BootstrapStep::PRE_RUNTIME_ORDER {
        log.push(step, StepOutcome::Ran);
    }
    log
}

#[test]
fn canonical_pre_runtime_order_verifies() {
    let log = pre_runtime_log();
    assert_eq!(log.verify_pre_runtime_order(), Ok(()));
    assert_eq!(log.len(), 4);
    assert!(!log.is_empty());
}

#[test]
fn phase_b_records_after_prefix_do_not_break_verification() {
    let mut log = pre_runtime_log();
    log.push(BootstrapStep::Runtime, StepOutcome::Ran);
    log.push_record(StepRecord::new(
        BootstrapStep::StartupJobs,
        StepOutcome::Skipped("disabled"),
    ));
    assert_eq!(log.verify_pre_runtime_order(), Ok(()));
}

#[test]
fn swapped_steps_fail_verification() {
    let mut log = StepLog::new();
    log.push(BootstrapStep::LegacyAppIdMigration, StepOutcome::Ran);
    log.push(BootstrapStep::FlatpakGate, StepOutcome::Ran);
    log.push(BootstrapStep::FlatpakMigration, StepOutcome::Ran);
    log.push(BootstrapStep::EnvPolicy, StepOutcome::Ran);
    let error = log.verify_pre_runtime_order().unwrap_err();
    assert_eq!(error.index, 0);
    assert_eq!(error.expected, BootstrapStep::FlatpakGate);
    assert_eq!(error.found, Some(BootstrapStep::LegacyAppIdMigration));
}

#[test]
fn missing_step_fails_verification() {
    let mut log = StepLog::new();
    log.push(BootstrapStep::FlatpakGate, StepOutcome::Ran);
    let error = log.verify_pre_runtime_order().unwrap_err();
    assert_eq!(error.index, 1);
    assert_eq!(error.expected, BootstrapStep::FlatpakMigration);
    assert_eq!(error.found, None);
}

#[test]
fn record_keeps_step_and_outcome() {
    let record = StepRecord::new(
        BootstrapStep::FlatpakMigration,
        StepOutcome::Degraded("migration failed"),
    );
    assert_eq!(record.step, BootstrapStep::FlatpakMigration);
    assert_eq!(record.outcome, StepOutcome::Degraded("migration failed"));
    assert_eq!(BootstrapStep::FlatpakMigration.name(), "flatpak_migration");
}
