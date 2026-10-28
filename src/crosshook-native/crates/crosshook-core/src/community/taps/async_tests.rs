use std::collections::BTreeMap;
use std::ffi::OsString;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tempfile::tempdir;

use super::git::git_security_env_pairs;
use super::runner::{GitInvocation, GitOutput, GitRunError, TapGitRunner};
use super::store::CommunityTapStore;
use super::types::{CommunityTapError, CommunityTapSubscription, CommunityTapSyncStatus};

const PIN: &str = "0123456789abcdef0123456789abcdef01234567";

enum Reply {
    Fail(&'static str),
    Io,
    Cancel,
    /// Runs `side_effect` (e.g. materialize a partial clone), then cancels.
    CancelWith(Box<dyn Fn() + Send + Sync>),
}

/// Records invocations; `script` maps an action to a reply (default: success).
struct FakeRunner {
    calls: Mutex<Vec<GitInvocation>>,
    script: Vec<(&'static str, Reply)>,
    cancelled: AtomicBool,
}

impl FakeRunner {
    fn new(script: Vec<(&'static str, Reply)>) -> Arc<Self> {
        Arc::new(Self {
            calls: Mutex::new(Vec::new()),
            script,
            cancelled: AtomicBool::new(false),
        })
    }

    fn actions(&self) -> Vec<&'static str> {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .map(|c| c.action)
            .collect()
    }
}

impl TapGitRunner for FakeRunner {
    fn run<'a>(
        &'a self,
        invocation: GitInvocation,
    ) -> Pin<Box<dyn Future<Output = Result<GitOutput, GitRunError>> + Send + 'a>> {
        Box::pin(async move {
            let action = invocation.action;
            self.calls.lock().unwrap().push(invocation);
            let reply = self
                .script
                .iter()
                .find(|(name, _)| *name == action)
                .map(|(_, reply)| reply);
            let output = |success, stdout: &str, stderr: &str| GitOutput {
                success,
                stdout: stdout.to_string(),
                stderr: stderr.to_string(),
            };
            match reply {
                Some(Reply::Fail(stderr)) => Ok(output(false, "", stderr)),
                Some(Reply::Io) => Err(GitRunError::Failed(std::io::Error::other("spawn"))),
                Some(Reply::Cancel) => {
                    self.cancelled.store(true, Ordering::SeqCst);
                    Err(GitRunError::Cancelled)
                }
                Some(Reply::CancelWith(side_effect)) => {
                    side_effect();
                    self.cancelled.store(true, Ordering::SeqCst);
                    Err(GitRunError::Cancelled)
                }
                None if action == "resolve tap commit" => Ok(output(true, "abc1234\n", "")),
                None => Ok(output(true, "", "")),
            }
        })
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

fn subscription(pinned: Option<&str>) -> CommunityTapSubscription {
    CommunityTapSubscription {
        extra: toml::Table::new(),
        url: "https://example.invalid/tap.git".to_string(),
        branch: Some("main".to_string()),
        pinned_commit: pinned.map(str::to_string),
    }
}

fn args(inv: &GitInvocation) -> Vec<String> {
    inv.args
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect()
}

#[tokio::test]
async fn clone_uses_separator_env_and_no_directory() {
    let temp = tempdir().unwrap();
    let store = CommunityTapStore::with_base_path(temp.path().join("store"));
    let runner = FakeRunner::new(vec![]);

    let result = store
        .sync_tap_with_runner(&subscription(None), runner.clone())
        .await
        .unwrap();

    assert_eq!(result.status, CommunityTapSyncStatus::Cloned);
    assert_eq!(result.head_commit, "abc1234");
    assert_eq!(result.last_sync_at, None);
    assert_eq!(
        runner.actions(),
        ["clone community tap", "resolve tap commit"]
    );

    let calls = runner.calls.lock().unwrap();
    let clone = &calls[0];
    let local = store.workspace_path(&subscription(None));
    assert_eq!(
        clone.args,
        vec![
            OsString::from("clone"),
            OsString::from("--branch"),
            OsString::from("main"),
            OsString::from("--single-branch"),
            OsString::from("--"),
            OsString::from("https://example.invalid/tap.git"),
            local.as_os_str().to_owned(),
        ]
    );
    assert_eq!(clone.directory, None);
    let expected_env: BTreeMap<String, String> = git_security_env_pairs()
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    assert_eq!(clone.env, expected_env);
    assert_eq!(calls[1].directory.as_deref(), Some(local.as_path()));
}

#[tokio::test]
async fn existing_clone_updates_with_reset_and_clean() {
    let temp = tempdir().unwrap();
    let store = CommunityTapStore::with_base_path(temp.path().join("store"));
    std::fs::create_dir_all(store.workspace_path(&subscription(None))).unwrap();
    let runner = FakeRunner::new(vec![]);

    let result = store
        .sync_tap_with_runner(&subscription(None), runner.clone())
        .await
        .unwrap();

    assert_eq!(result.status, CommunityTapSyncStatus::Updated);
    assert_eq!(
        runner.actions(),
        [
            "fetch community tap",
            "reset community tap",
            "clean community tap",
            "resolve tap commit"
        ]
    );
    let calls = runner.calls.lock().unwrap();
    assert_eq!(
        args(&calls[0]),
        ["fetch", "--prune", "origin", "--", "main"]
    );
}

#[tokio::test]
async fn pinned_tap_fetches_checks_out_and_cleans() {
    let temp = tempdir().unwrap();
    let store = CommunityTapStore::with_base_path(temp.path().join("store"));
    std::fs::create_dir_all(store.workspace_path(&subscription(Some(PIN)))).unwrap();
    let runner = FakeRunner::new(vec![]);

    store
        .sync_tap_with_runner(&subscription(Some(PIN)), runner.clone())
        .await
        .unwrap();

    assert_eq!(
        runner.actions(),
        [
            "fetch community tap",
            "checkout pinned commit",
            "clean community tap",
            "resolve tap commit"
        ]
    );
    assert_eq!(
        args(&runner.calls.lock().unwrap()[1]),
        ["checkout", "--detach", PIN]
    );
}

#[tokio::test]
async fn pinned_clone_checks_out_after_clone() {
    let temp = tempdir().unwrap();
    let store = CommunityTapStore::with_base_path(temp.path().join("store"));
    let runner = FakeRunner::new(vec![]);

    store
        .sync_tap_with_runner(&subscription(Some(PIN)), runner.clone())
        .await
        .unwrap();

    assert_eq!(
        runner.actions(),
        [
            "clone community tap",
            "checkout pinned commit",
            "resolve tap commit"
        ]
    );
}

#[tokio::test]
async fn failed_fetch_falls_back_only_for_existing_clone() {
    let temp = tempdir().unwrap();
    let store = CommunityTapStore::with_base_path(temp.path().join("store"));
    let sub = subscription(None);

    // Existing clone: failed fetch (non-zero exit, then spawn failure) -> cache fallback.
    std::fs::create_dir_all(store.workspace_path(&sub)).unwrap();
    for reply in [Reply::Fail("offline"), Reply::Io] {
        let runner = FakeRunner::new(vec![("fetch community tap", reply)]);
        let result = store
            .sync_tap_with_runner(&sub, runner.clone())
            .await
            .unwrap();
        assert_eq!(result.status, CommunityTapSyncStatus::CachedFallback);
        assert!(result.from_cache);
        assert_eq!(
            runner.actions(),
            ["fetch community tap", "resolve tap commit"]
        );
    }

    // Missing clone: a failed clone is a hard error.
    let other = CommunityTapStore::with_base_path(temp.path().join("other"));
    let runner = FakeRunner::new(vec![("clone community tap", Reply::Fail("nope"))]);
    let err = other
        .sync_tap_with_runner(&sub, runner.clone())
        .await
        .unwrap_err();
    assert!(matches!(err, CommunityTapError::Git { .. }), "{err}");
    assert_eq!(runner.actions(), ["clone community tap"]);
}

#[tokio::test]
async fn cancellation_during_fetch_aborts_without_fallback_or_index() {
    let temp = tempdir().unwrap();
    let store = CommunityTapStore::with_base_path(temp.path().join("store"));
    let sub = subscription(None);
    std::fs::create_dir_all(store.workspace_path(&sub)).unwrap();
    let runner = FakeRunner::new(vec![("fetch community tap", Reply::Cancel)]);

    let err = store
        .sync_tap_with_runner(&sub, runner.clone())
        .await
        .unwrap_err();

    assert!(matches!(err, CommunityTapError::Cancelled), "{err}");
    assert_eq!(err.to_string(), "community tap sync was cancelled");
    assert_eq!(runner.actions(), ["fetch community tap"]);
}

#[tokio::test]
async fn cancellation_before_start_runs_no_git() {
    let temp = tempdir().unwrap();
    let store = CommunityTapStore::with_base_path(temp.path().join("store"));
    let runner = FakeRunner::new(vec![]);
    runner.cancelled.store(true, Ordering::SeqCst);

    let err = store
        .sync_tap_with_runner(&subscription(None), runner.clone())
        .await
        .unwrap_err();

    assert!(matches!(err, CommunityTapError::Cancelled));
    assert!(runner.actions().is_empty());
}

#[tokio::test]
async fn cancelled_clone_removes_partial_workspace() {
    let temp = tempdir().unwrap();
    let store = CommunityTapStore::with_base_path(temp.path().join("store"));
    let sub = subscription(None);
    let local = store.workspace_path(&sub);
    // The clone step materializes a partial workspace, then reports
    // cancellation (as a SIGKILLed `git clone` would).
    let marker = local.clone();
    let runner = FakeRunner::new(vec![(
        "clone community tap",
        Reply::CancelWith(Box::new(move || {
            std::fs::create_dir_all(marker.join(".git")).unwrap();
            std::fs::write(marker.join("partial"), "truncated").unwrap();
        })),
    )]);

    let err = store
        .sync_tap_with_runner(&sub, runner.clone())
        .await
        .unwrap_err();

    assert!(matches!(err, CommunityTapError::Cancelled));
    assert_eq!(runner.actions(), ["clone community tap"]);
    assert!(
        !local.exists(),
        "a cancelled first clone must remove its partial workspace"
    );
}

#[tokio::test]
async fn invalid_inputs_are_rejected_before_any_invocation() {
    let temp = tempdir().unwrap();
    let store = CommunityTapStore::with_base_path(temp.path().join("store"));
    let runner = FakeRunner::new(vec![]);

    let mut bad_url = subscription(None);
    bad_url.url = "file:///etc".to_string();
    let mut bad_branch = subscription(None);
    bad_branch.branch = Some("--upload-pack=x".to_string());

    assert!(matches!(
        store.sync_tap_with_runner(&bad_url, runner.clone()).await,
        Err(CommunityTapError::InvalidTapUrl(_))
    ));
    assert!(matches!(
        store
            .sync_tap_with_runner(&bad_branch, runner.clone())
            .await,
        Err(CommunityTapError::InvalidBranch(_))
    ));
    assert!(runner.actions().is_empty());
}

#[tokio::test]
async fn invalid_pin_passes_normalize_but_fails_at_checkout() {
    let temp = tempdir().unwrap();
    let store = CommunityTapStore::with_base_path(temp.path().join("store"));
    // No runner is used: normalize no longer rejects the pin, and checkout-time
    // validation happens while building the clone steps, before any git run.
    let runner = FakeRunner::new(vec![]);
    let bad_pin = subscription(Some("'; rm -rf /"));

    // The stored/loaded pin must not break resolution, listing or import.
    let normalized = CommunityTapStore::normalize_and_validate(&bad_pin).unwrap();
    assert_eq!(normalized.pinned_commit.as_deref(), Some("'; rm -rf /"));

    // The pin is still rejected when a sync reaches the pinned checkout step.
    let err = store
        .sync_tap_with_runner(&bad_pin, runner.clone())
        .await
        .unwrap_err();
    assert!(matches!(err, CommunityTapError::InvalidPinnedCommit(_)));
    assert!(runner.actions().is_empty());
}

#[tokio::test]
async fn sync_many_aborts_on_first_hard_error() {
    let temp = tempdir().unwrap();
    let store = CommunityTapStore::with_base_path(temp.path().join("store"));
    let mut second = subscription(None);
    second.url = "https://example.invalid/second.git".to_string();
    let runner = FakeRunner::new(vec![("clone community tap", Reply::Fail("boom"))]);

    let err = store
        .sync_many_with_runner(&[subscription(None), second], runner.clone())
        .await
        .unwrap_err();

    assert!(matches!(err, CommunityTapError::Git { .. }));
    assert_eq!(runner.actions(), ["clone community tap"]);
}
