use super::super::messages::MSG_OP_NOT_FOUND;
use super::super::owners::OwnerLedger;
use super::helpers::owner;
use crate::jobs::JobId;
use crate::AppErrorKind;

fn id(value: &str) -> JobId {
    JobId::from_string(value)
}

#[test]
fn authorize_returns_the_job_id_for_the_owner() {
    let ledger = OwnerLedger::default();
    let _guard = ledger.register(id("job-1"), owner("main"));
    assert_eq!(
        ledger.authorize(&owner("main"), "job-1").unwrap(),
        id("job-1")
    );
}

#[test]
fn authorize_foreign_unknown_and_empty_share_one_error() {
    let ledger = OwnerLedger::default();
    let _guard = ledger.register(id("job-1"), owner("main"));
    let foreign = ledger.authorize(&owner("other"), "job-1").unwrap_err();
    let unknown = ledger.authorize(&owner("main"), "job-2").unwrap_err();
    let empty = ledger.authorize(&owner("main"), "").unwrap_err();
    assert_eq!(foreign.kind(), AppErrorKind::NotFound);
    assert_eq!(foreign.message(), MSG_OP_NOT_FOUND);
    assert_eq!(foreign, unknown);
    assert_eq!(foreign, empty);
}

#[test]
fn guard_removes_its_entry_on_drop() {
    let ledger = OwnerLedger::default();
    let guard = ledger.register(id("job-1"), owner("main"));
    assert_eq!(ledger.len(), 1);
    drop(guard);
    assert_eq!(ledger.len(), 0);
    assert_eq!(
        ledger
            .authorize(&owner("main"), "job-1")
            .unwrap_err()
            .message(),
        MSG_OP_NOT_FOUND
    );
}

#[test]
fn guard_only_removes_its_own_entry() {
    let ledger = OwnerLedger::default();
    let first = ledger.register(id("job-1"), owner("main"));
    let _second = ledger.register(id("job-2"), owner("main"));
    drop(first);
    assert_eq!(ledger.len(), 1);
    assert!(ledger.authorize(&owner("main"), "job-2").is_ok());
}

#[test]
fn guard_removes_its_entry_when_the_holder_panics() {
    let ledger = OwnerLedger::default();
    let held = ledger.clone();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _guard = held.register(id("job-1"), owner("main"));
        panic!("job body panicked");
    }));
    assert!(result.is_err());
    assert_eq!(ledger.len(), 0);
}
