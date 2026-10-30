//! Owner slots: one slot per owner, owner-checked cancel and kill, a cancel
//! before the bind, and slot release on drop.

use tokio::sync::oneshot;

use crate::jobs::{JobId, JobRegistry, JobSpec, Outcome};
use crate::services::process::owner::{ActiveJobs, JobOwner, MSG_OWNER_BUSY};
use crate::AppErrorKind;

#[test]
fn a_second_claim_of_the_same_owner_conflicts_until_the_first_is_released() {
    let active = ActiveJobs::default();
    let owner = JobOwner::new("main");

    let first = active.claim(owner.clone()).unwrap();
    let error = active.claim(owner.clone()).err().expect("owner is busy");
    assert_eq!(error.kind(), AppErrorKind::Conflict);
    assert_eq!(error.message(), MSG_OWNER_BUSY);
    // Another owner is independent.
    let _other = active.claim(JobOwner::new("other")).unwrap();
    assert_eq!(active.len(), 2);

    drop(first);
    assert_eq!(active.len(), 1);
    let _again = active.claim(owner).unwrap();
}

#[test]
fn kill_fires_only_the_owner_token() {
    let active = ActiveJobs::default();
    let slot = active.claim(JobOwner::new("main")).unwrap();
    let other = active.claim(JobOwner::new("other")).unwrap();

    assert!(!active.kill(&JobOwner::new("stranger")));
    assert!(active.kill(&JobOwner::new("main")));
    assert!(slot.kill_token().is_cancelled());
    assert!(!other.kill_token().is_cancelled());

    drop(slot);
    assert!(!active.kill(&JobOwner::new("main")));
}

#[test]
fn a_cancel_before_the_bind_is_reported_by_the_bind() {
    let active = ActiveJobs::default();
    let jobs = JobRegistry::default();
    let slot = active.claim(JobOwner::new("main")).unwrap();

    assert!(active.cancel(&JobOwner::new("main"), &jobs));
    assert!(slot.bind(&JobId::from_string("job-1")));
    assert!(!active.cancel(&JobOwner::new("stranger"), &jobs));
}

#[tokio::test]
async fn a_cancel_after_the_bind_cancels_the_bound_job_only() {
    let active = ActiveJobs::default();
    let jobs = JobRegistry::default();
    let slot = active.claim(JobOwner::new("main")).unwrap();
    let (id_tx, id_rx) = oneshot::channel();
    let handle = jobs
        .start(JobSpec::new("test-owner"), move |ctx| async move {
            let _ = id_tx.send(ctx.id().clone());
            ctx.cancelled().await;
            Outcome::Cancelled
        })
        .unwrap();
    let id = id_rx.await.unwrap();
    assert!(!slot.bind(&id));

    assert!(!active.cancel(&JobOwner::new("stranger"), &jobs));
    assert!(active.cancel(&JobOwner::new("main"), &jobs));
    let mut events = handle.into_events();
    while events.recv().await.is_some() {}
    jobs.reap(&id).unwrap();
    assert_eq!(jobs.job_counts(), (0, 0));
}
