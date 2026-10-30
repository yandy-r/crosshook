//! `start`/`cancel`: live streaming then completion, owner-checked cancel,
//! prefix exclusivity, validation and the response wire shape.

use std::path::Path;

use crosshook_core::update::UpdateGameResult;
use tokio::sync::oneshot;

use super::{drain, next_line, open_gate, Fixture};
use crate::jobs::{JobSpec, Outcome};
use crate::services::process::owner::MSG_OWNER_BUSY;
use crate::services::process::{
    prefix_job_key, prefix_job_key_for_path, JobOwner, MSG_PREFIX_BUSY,
};
use crate::services::update::{UpdateCompletion, UpdateGameResponse};
use crate::AppErrorKind;

fn gate_script(fifo: &Path, before: &str, after: &str) -> String {
    format!("{before}; read x < '{}'; {after}", fifo.display())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lines_stream_then_completion_reports_exit_code_zero() {
    let fixture = Fixture::new();
    let fifo = fixture.fifo();
    fixture.install_proton(&gate_script(&fifo, "echo one", "echo two >&2"));

    let (response, mut run) = fixture
        .service
        .start(JobOwner::new("main"), fixture.request())
        .await
        .unwrap();

    assert_eq!(response.run_id, run.run_id());
    assert!(response.result.succeeded);
    assert_eq!(response.result.message, "Update process launched.");
    // The updater is blocked on the gate: this line arrived before exit.
    assert_eq!(next_line(&mut run).await, "one");
    open_gate(fifo).await;
    let (rest, completion) = drain(&mut run).await;
    assert_eq!(rest, vec!["two".to_string()]);
    assert_eq!(
        completion,
        Some(UpdateCompletion {
            code: Some(0),
            signal: None
        })
    );
    assert_eq!(run.next().await, None);
    let log = std::fs::read_to_string(&response.result.helper_log_path).unwrap();
    assert_eq!(log, "one\ntwo\n");
    assert!(Path::new(&response.result.helper_log_path).starts_with(&fixture.log_dir));
    assert_eq!(fixture.jobs.job_counts(), (0, 0));
    assert_eq!(fixture.service.active.len(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failing_updater_reports_its_exit_code() {
    let fixture = Fixture::new();
    fixture.install_proton("echo failing; exit 3");

    let (_, mut run) = fixture
        .service
        .start(JobOwner::new("main"), fixture.request())
        .await
        .unwrap();

    let (lines, completion) = drain(&mut run).await;
    // `bad-args` (exit 97) would mean the command line or env was wrong.
    assert_eq!(lines, vec!["failing".to_string()]);
    assert_eq!(
        completion,
        Some(UpdateCompletion {
            code: Some(3),
            signal: None
        })
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_by_owner_terminates_with_sigterm() {
    let fixture = Fixture::new();
    let fifo = fixture.fifo();
    // Nobody opens the gate: only the cancel can end the updater.
    fixture.install_proton(&gate_script(&fifo, "echo ready", "echo unreachable"));
    let owner = JobOwner::new("main");

    let (_, mut run) = fixture
        .service
        .start(owner.clone(), fixture.request())
        .await
        .unwrap();
    assert_eq!(next_line(&mut run).await, "ready");

    assert!(fixture.service.cancel(&owner));
    let (lines, completion) = drain(&mut run).await;
    assert!(lines.is_empty(), "unexpected output: {lines:?}");
    assert_eq!(
        completion,
        Some(UpdateCompletion {
            code: None,
            signal: Some(15)
        })
    );
    assert_eq!(fixture.jobs.job_counts(), (0, 0));
    // The slot is gone with the job: a second cancel has nothing to cancel.
    assert!(!fixture.service.cancel(&owner));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_by_foreign_owner_is_refused_and_the_job_keeps_running() {
    let fixture = Fixture::new();
    let fifo = fixture.fifo();
    fixture.install_proton(&gate_script(&fifo, "echo ready", "echo done"));

    let (_, mut run) = fixture
        .service
        .start(JobOwner::new("main"), fixture.request())
        .await
        .unwrap();
    assert_eq!(next_line(&mut run).await, "ready");

    assert!(!fixture.service.cancel(&JobOwner::new("other-window")));
    assert_eq!(fixture.jobs.job_counts(), (1, 0));

    // Still alive: it reads the gate and exits naturally.
    open_gate(fifo).await;
    let (lines, completion) = drain(&mut run).await;
    assert_eq!(lines, vec!["done".to_string()]);
    assert_eq!(
        completion,
        Some(UpdateCompletion {
            code: Some(0),
            signal: None
        })
    );
}

#[tokio::test]
async fn cancel_without_an_active_job_is_false() {
    let fixture = Fixture::new();
    assert!(!fixture.service.cancel(&JobOwner::new("main")));
    assert_eq!(fixture.jobs.job_counts(), (0, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn held_prefix_key_conflicts_and_nothing_runs() {
    let fixture = Fixture::new();
    // The fake would record any invocation.
    let invoked = fixture.dir.path().join("invoked");
    fixture.install_proton(&format!("touch '{}'", invoked.display()));
    // Another prefix operation (e.g. a prefix-dependency install) holds the
    // key of the same prefix.
    let (_, key) = prefix_job_key(fixture.prefix.to_str().unwrap()).unwrap();
    let (release_tx, release_rx) = oneshot::channel::<()>();
    let holder = fixture
        .jobs
        .start(
            JobSpec::new("prefix-deps-install").with_key(key.clone()),
            move |_| async move {
                let _ = release_rx.await;
                Outcome::Succeeded
            },
        )
        .unwrap();
    assert!(fixture.jobs.is_key_held(&key));

    // Both the configured path and a symlink alias resolve to the held key.
    let alias = fixture.dir.path().join("prefix-alias");
    std::os::unix::fs::symlink(&fixture.prefix, &alias).unwrap();
    for prefix_path in [fixture.prefix.clone(), alias] {
        let mut request = fixture.request();
        request.prefix_path = prefix_path.display().to_string();
        let error = fixture
            .service
            .start(JobOwner::new("main"), request)
            .await
            .err()
            .expect("a held prefix conflicts");
        assert_eq!(error.kind(), AppErrorKind::Conflict);
        assert_eq!(error.message(), MSG_PREFIX_BUSY);
    }

    assert!(!invoked.exists(), "the updater must not run");
    assert!(fixture.log_files().is_empty(), "no log is left behind");
    assert_eq!(fixture.jobs.job_counts(), (1, 0));
    assert_eq!(fixture.service.active.len(), 0);
    release_tx.send(()).unwrap();
    let id = holder.id().clone();
    let mut events = holder.into_events();
    while events.recv().await.is_some() {}
    fixture.jobs.reap(&id).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_second_start_of_the_same_window_conflicts_while_its_update_is_active() {
    let fixture = Fixture::new();
    let fifo = fixture.fifo();
    fixture.install_proton(&gate_script(&fifo, "echo ready", "echo done"));
    let owner = JobOwner::new("main");
    let (_, mut run) = fixture
        .service
        .start(owner.clone(), fixture.request())
        .await
        .unwrap();
    assert_eq!(next_line(&mut run).await, "ready");

    let error = fixture
        .service
        .start(owner.clone(), fixture.request())
        .await
        .err()
        .expect("one active update per window");
    assert_eq!(error.kind(), AppErrorKind::Conflict);
    assert_eq!(error.message(), MSG_OWNER_BUSY);
    assert_eq!(
        fixture.log_files().len(),
        1,
        "the refused start left no log"
    );
    // The first update is still the window's: its cancel reaches it.
    assert!(fixture.service.cancel(&owner));
    let (_, completion) = drain(&mut run).await;
    assert_eq!(
        completion,
        Some(UpdateCompletion {
            code: None,
            signal: Some(15)
        })
    );
    assert_eq!(fixture.service.active.len(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_fresh_prefix_conflicts_with_a_job_on_its_future_pfx() {
    let fixture = Fixture::new();
    let invoked = fixture.dir.path().join("invoked");
    fixture.install_proton(&format!("touch '{}'", invoked.display()));
    // The fixture prefix has no `pfx` yet; a game install holds the `pfx`
    // Proton will create there.
    let key = prefix_job_key_for_path(&fixture.prefix.join("pfx"));
    let (release_tx, release_rx) = oneshot::channel::<()>();
    let holder = fixture
        .jobs
        .start(
            JobSpec::new("install-game").with_key(key),
            move |_| async move {
                let _ = release_rx.await;
                Outcome::Succeeded
            },
        )
        .unwrap();

    let error = fixture
        .service
        .start(JobOwner::new("main"), fixture.request())
        .await
        .err()
        .expect("the fresh prefix is busy");
    assert_eq!(error.kind(), AppErrorKind::Conflict);
    assert_eq!(error.message(), MSG_PREFIX_BUSY);
    assert!(!invoked.exists(), "the updater must not run");

    release_tx.send(()).unwrap();
    let id = holder.id().clone();
    let mut events = holder.into_events();
    while events.recv().await.is_some() {}
    fixture.jobs.reap(&id).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_spawn_failure_fails_the_start_like_before() {
    let fixture = Fixture::new();
    // Validation passes (an executable file) but the exec fails.
    super::write_executable(&fixture.proton, "#!/nonexistent/interpreter\n");

    let error = fixture
        .service
        .start(JobOwner::new("main"), fixture.request())
        .await
        .err()
        .expect("spawn failure fails the start");
    assert_eq!(error.kind(), AppErrorKind::Io);
    assert!(
        error
            .message()
            .starts_with("Failed to launch the updater through Proton: "),
        "unexpected text: {}",
        error.message()
    );
    assert!(fixture.log_files().is_empty(), "unused log removed");
    assert_eq!(fixture.jobs.job_counts(), (0, 0));
    assert_eq!(fixture.service.active.len(), 0);
}

#[tokio::test]
async fn invalid_request_is_a_validation_error_and_nothing_runs() {
    let fixture = Fixture::new();
    fixture.install_proton("exit 0");
    let mut request = fixture.request();
    request.updater_path = fixture.dir.path().join("missing.exe").display().to_string();

    let error = fixture
        .service
        .start(JobOwner::new("main"), request.clone())
        .await
        .err()
        .expect("invalid request");
    assert_eq!(
        error.kind(),
        AppErrorKind::Validation {
            field: "updater_path"
        }
    );
    assert_eq!(
        error.message(),
        "The updater executable path does not exist."
    );
    assert_eq!(fixture.jobs.job_counts(), (0, 0));
    assert!(fixture.log_files().is_empty());

    let validated = fixture.service.validate(request).await.unwrap_err();
    assert_eq!(validated, error);
    fixture.service.validate(fixture.request()).await.unwrap();
}

#[test]
fn response_json_is_the_old_result_plus_run_id() {
    let result = UpdateGameResult {
        succeeded: true,
        message: "Update process launched.".to_string(),
        helper_log_path: "/tmp/crosshook-logs/update-x.log".to_string(),
    };
    let mut expected = serde_json::to_value(&result).unwrap();
    expected
        .as_object_mut()
        .unwrap()
        .insert("run_id".to_string(), serde_json::json!("run-1"));
    let response = UpdateGameResponse {
        result,
        run_id: "run-1".to_string(),
    };

    let value = serde_json::to_value(&response).unwrap();
    assert_eq!(
        value,
        serde_json::json!({
            "succeeded": true,
            "message": "Update process launched.",
            "helper_log_path": "/tmp/crosshook-logs/update-x.log",
            "run_id": "run-1",
        })
    );
    assert_eq!(value, expected);
    let back: UpdateGameResponse = serde_json::from_value(value).unwrap();
    assert_eq!(back, response);
}
