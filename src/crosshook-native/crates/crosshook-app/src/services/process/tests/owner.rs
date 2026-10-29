//! Owner slot: register, get, and guard removal of the matching id only.

use tokio_util::sync::CancellationToken;

use crate::jobs::JobId;
use crate::services::process::owner::{ActiveJob, ActiveJobs, JobOwner};

fn job(id: &str) -> (ActiveJob, CancellationToken) {
    let kill = CancellationToken::new();
    (
        ActiveJob {
            id: JobId::from_string(id),
            kill: kill.clone(),
        },
        kill,
    )
}

#[test]
fn register_and_get_return_the_owner_entry_only() {
    let active = ActiveJobs::default();
    let owner = JobOwner::new("main");
    let foreign = JobOwner::new("other");
    let (entry, kill) = job("job-1");

    let guard = active.register(owner.clone(), entry);
    let (id, token) = active.get(&owner).expect("owner has an active job");
    assert_eq!(id, JobId::from_string("job-1"));
    // The stored token is the registered one: firing it reaches the job.
    token.cancel();
    assert!(kill.is_cancelled());
    assert!(active.get(&foreign).is_none());
    assert_eq!(active.len(), 1);

    drop(guard);
    assert!(active.get(&owner).is_none());
    assert_eq!(active.len(), 0);
}

#[test]
fn stale_guard_does_not_remove_a_newer_entry() {
    let active = ActiveJobs::default();
    let owner = JobOwner::new("main");
    let (first, _) = job("job-1");
    let (second, _) = job("job-2");

    let old_guard = active.register(owner.clone(), first);
    let new_guard = active.register(owner.clone(), second);
    assert_eq!(active.get(&owner).unwrap().0, JobId::from_string("job-2"));

    // The replaced job finishing must leave the newer registration alone.
    drop(old_guard);
    assert_eq!(active.get(&owner).unwrap().0, JobId::from_string("job-2"));

    drop(new_guard);
    assert!(active.get(&owner).is_none());
}

#[test]
fn owners_are_independent() {
    let active = ActiveJobs::default();
    let (a, _) = job("job-a");
    let (b, _) = job("job-b");
    let guard_a = active.register(JobOwner::new("a"), a);
    let _guard_b = active.register(JobOwner::new("b"), b);
    drop(guard_a);
    assert!(active.get(&JobOwner::new("a")).is_none());
    assert_eq!(
        active.get(&JobOwner::new("b")).unwrap().0,
        JobId::from_string("job-b")
    );
}

#[test]
fn clones_share_entries() {
    let active = ActiveJobs::default();
    let clone = active.clone();
    let (entry, _) = job("job-1");
    let _guard = active.register(JobOwner::new("main"), entry);
    assert!(clone.get(&JobOwner::new("main")).is_some());
}
