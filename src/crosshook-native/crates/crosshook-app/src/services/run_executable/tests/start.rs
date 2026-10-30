//! `start`: unique throwaway prefixes, key held through cleanup, user
//! prefixes kept, conflicts and validation errors.

use std::path::Path;

use crosshook_core::run_executable::RunExecutableValidationError;

use super::{key_of, open_gate, Fixture, GATED};
use crate::jobs::{JobSpec, Outcome};
use crate::services::process::owner::MSG_OWNER_BUSY;
use crate::services::process::{prefix_job_key_for_path, JobOwner, MSG_PREFIX_BUSY};
use crate::services::run_executable::{RunCompletion, RunExecutableStartError};
use crate::AppErrorKind;

const NATURAL_EXIT: RunCompletion = RunCompletion {
    code: Some(0),
    signal: None,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn same_stem_throwaway_runs_get_distinct_dirs_and_both_run() {
    let fixture = Fixture::new();
    fixture.install_proton(GATED);

    // Two windows (one active run per window).
    let (first, mut first_run) = fixture.start("main", fixture.request()).await;
    let (second, mut second_run) = fixture.start("second", fixture.request()).await;
    let first_prefix = first.result.resolved_prefix_path.clone();
    let second_prefix = second.result.resolved_prefix_path.clone();

    assert_ne!(first_prefix, second_prefix);
    for prefix in [&first_prefix, &second_prefix] {
        let path = Path::new(prefix);
        assert_eq!(path.parent(), Some(fixture.adhoc_root.as_path()));
        let name = path.file_name().unwrap().to_str().unwrap();
        let suffix = name.strip_prefix("setup-").expect("slug of the exe stem");
        assert_eq!(suffix.len(), 12);
        assert!(suffix.bytes().all(|b| b.is_ascii_hexdigit()));
    }
    assert_ne!(first.run_id, second.run_id);
    assert_eq!(first.run_id, first_run.run_id());
    assert!(first.result.succeeded);
    assert_eq!(first.result.message, "Executable launched.");

    // Both are running at once, each in its own prefix.
    first_run.wait_ready(&first_prefix).await;
    second_run.wait_ready(&second_prefix).await;
    assert_eq!(fixture.throwaway_dirs().len(), 2);

    open_gate(&first_prefix).await;
    let (lines, completion) = first_run.drain().await;
    assert_eq!(lines, vec!["done".to_string()]);
    assert_eq!(completion, Some(NATURAL_EXIT));
    // The first run's cleanup left the still-running second prefix alone.
    assert!(!Path::new(&first_prefix).exists());
    assert!(Path::new(&second_prefix).join("drive_c/file.txt").exists());

    open_gate(&second_prefix).await;
    let (_, completion) = second_run.drain().await;
    assert_eq!(completion, Some(NATURAL_EXIT));
    assert!(fixture.throwaway_dirs().is_empty());
    assert_eq!(fixture.jobs.job_counts(), (0, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn key_is_held_until_cleanup_finished_and_the_completion_arrives() {
    let fixture = Fixture::new();
    fixture.install_proton(GATED);
    let (response, mut run) = fixture.start("main", fixture.request()).await;
    let prefix = response.result.resolved_prefix_path.clone();
    let key = key_of(&prefix);

    run.wait_ready(&prefix).await;
    assert!(fixture.jobs.is_key_held(&key));
    open_gate(&prefix).await;
    assert_eq!(run.next_line().await, "done");
    // The child is done, but the job has not completed: it still holds the
    // key while it removes the prefix.
    let (_, completion) = run.drain().await;
    assert_eq!(completion, Some(NATURAL_EXIT));
    // By the time the completion is observed the prefix is already gone,
    // so the cleanup ran inside the job, before the key was released.
    assert!(!Path::new(&prefix).exists());
    assert!(!fixture.jobs.is_key_held(&key));

    let log = std::fs::read_to_string(&response.result.helper_log_path).unwrap();
    assert_eq!(log, format!("prefix={prefix}\nready\ndone\n"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_failing_executable_reports_its_code_and_still_cleans_up() {
    let fixture = Fixture::new();
    fixture.install_proton("echo oops >&2\nexit 3");
    let (response, mut run) = fixture.start("main", fixture.request()).await;
    let prefix = response.result.resolved_prefix_path.clone();

    let (lines, completion) = run.drain().await;
    assert_eq!(lines, vec![format!("prefix={prefix}"), "oops".to_string()]);
    assert_eq!(
        completion,
        Some(RunCompletion {
            code: Some(3),
            signal: None
        })
    );
    assert!(!Path::new(&prefix).exists());
    assert_eq!(fixture.jobs.job_counts(), (0, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_explicit_user_prefix_is_never_removed() {
    let fixture = Fixture::new();
    fixture.install_proton("exit 0");
    // Even a user prefix that sits inside the throwaway namespace is kept:
    // only prefixes the service resolved itself are deleted.
    std::fs::create_dir_all(&fixture.adhoc_root).unwrap();
    let inside_root = fixture.adhoc_root.join("chosen-by-user");
    let elsewhere = fixture.root.join("my-prefix");
    for prefix in [&inside_root, &elsewhere] {
        std::fs::create_dir(prefix).unwrap();
        let (response, mut run) = fixture
            .start("main", fixture.request_with_prefix(prefix))
            .await;
        assert_eq!(
            response.result.resolved_prefix_path,
            prefix.display().to_string()
        );

        let (lines, completion) = run.drain().await;
        assert_eq!(lines, vec![format!("prefix={}", prefix.display())]);
        assert_eq!(completion, Some(NATURAL_EXIT));
        assert!(prefix.join("drive_c/file.txt").exists());
    }
    assert_eq!(fixture.jobs.job_counts(), (0, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_held_prefix_gives_conflict_and_runs_nothing() {
    let fixture = Fixture::new();
    fixture.install_proton(GATED);
    let prefix = fixture.root.join("shared");
    std::fs::create_dir(&prefix).unwrap();
    let prefix_text = prefix.display().to_string();
    let (_, mut run) = fixture
        .start("main", fixture.request_with_prefix(&prefix))
        .await;
    run.wait_ready(&prefix_text).await;
    let logs_before = fixture.log_files().len();

    let error = fixture
        .service
        .start(JobOwner::new("other"), fixture.request_with_prefix(&prefix))
        .await
        .err()
        .expect("second run on the same prefix is refused");
    let error = match error {
        RunExecutableStartError::Failed(error) => error,
        other => panic!("expected Failed(Conflict), got {other:?}"),
    };
    assert_eq!(error.kind(), AppErrorKind::Conflict);
    assert_eq!(error.message(), MSG_PREFIX_BUSY);
    // The refused attempt left no log behind and did not touch the prefix.
    assert_eq!(fixture.log_files().len(), logs_before);
    assert_eq!(fixture.jobs.job_counts(), (1, 0));

    open_gate(&prefix_text).await;
    let (_, completion) = run.drain().await;
    assert_eq!(completion, Some(NATURAL_EXIT));
    assert!(prefix.join("drive_c/file.txt").exists());
    assert_eq!(fixture.jobs.job_counts(), (0, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn validation_errors_keep_their_variant_and_run_nothing() {
    let fixture = Fixture::new();
    fixture.install_proton("exit 0");
    let mut request = fixture.request();
    request.executable_path = fixture.root.join("missing.exe").display().to_string();

    let error = fixture
        .service
        .start(JobOwner::new("main"), request.clone())
        .await
        .err()
        .expect("invalid request is refused");
    assert_eq!(
        error,
        RunExecutableStartError::Validation(RunExecutableValidationError::ExecutablePathMissing)
    );
    assert_eq!(
        fixture.service.validate(request).await,
        Err(RunExecutableStartError::Validation(
            RunExecutableValidationError::ExecutablePathMissing
        ))
    );
    assert_eq!(fixture.service.validate(fixture.request()).await, Ok(()));

    let mut not_windows = fixture.request();
    not_windows.executable_path = fixture.proton.display().to_string();
    assert_eq!(
        fixture
            .service
            .start(JobOwner::new("main"), not_windows)
            .await
            .err(),
        Some(RunExecutableStartError::Validation(
            RunExecutableValidationError::ExecutablePathNotWindowsExecutable
        ))
    );
    assert!(fixture.throwaway_dirs().is_empty());
    assert!(fixture.log_files().is_empty());
    assert_eq!(fixture.jobs.job_counts(), (0, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_second_start_of_the_same_window_conflicts_while_its_run_is_active() {
    let fixture = Fixture::new();
    fixture.install_proton(GATED);
    let (response, mut run) = fixture.start("main", fixture.request()).await;
    let prefix = response.result.resolved_prefix_path.clone();
    run.wait_ready(&prefix).await;

    let error = fixture
        .service
        .start(JobOwner::new("main"), fixture.request())
        .await
        .err()
        .expect("one active run per window");
    let error = match error {
        RunExecutableStartError::Failed(error) => error,
        other => panic!("expected Failed(Conflict), got {other:?}"),
    };
    assert_eq!(error.kind(), AppErrorKind::Conflict);
    assert_eq!(error.message(), MSG_OWNER_BUSY);
    // The refused start provisioned nothing and the first run stays
    // reachable by its window.
    assert_eq!(fixture.throwaway_dirs().len(), 1);
    assert_eq!(fixture.log_files().len(), 1);

    open_gate(&prefix).await;
    let (_, completion) = run.drain().await;
    assert_eq!(completion, Some(NATURAL_EXIT));
    // Finished: the window may start again.
    fixture.install_proton("exit 0");
    let (_, mut next) = fixture.start("main", fixture.request()).await;
    assert_eq!(next.drain().await.1, Some(NATURAL_EXIT));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_fresh_prefix_shares_the_key_of_its_future_pfx() {
    let fixture = Fixture::new();
    fixture.install_proton("exit 0");
    // A compat data root Proton has not initialized yet (no `pfx`).
    let fresh = fixture.root.join("fresh");
    std::fs::create_dir(&fresh).unwrap();
    // Another operation holds the prefix through its `pfx` form (e.g. a
    // game install that anticipates the `pfx` Proton creates).
    let key = prefix_job_key_for_path(&fresh.join("pfx"));
    let (release_tx, release_rx) = tokio::sync::oneshot::channel::<()>();
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
        .start(JobOwner::new("main"), fixture.request_with_prefix(&fresh))
        .await
        .err()
        .expect("the fresh prefix is busy");
    let error = match error {
        RunExecutableStartError::Failed(error) => error,
        other => panic!("expected Failed(Conflict), got {other:?}"),
    };
    assert_eq!(error.kind(), AppErrorKind::Conflict);
    assert_eq!(error.message(), MSG_PREFIX_BUSY);
    assert!(!fresh.join("drive_c").exists(), "nothing ran");

    release_tx.send(()).unwrap();
    let id = holder.id().clone();
    let mut events = holder.into_events();
    while events.recv().await.is_some() {}
    fixture.jobs.reap(&id).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_spawn_failure_fails_the_start_and_cleans_up() {
    let fixture = Fixture::new();
    // Proton passes validation but cannot be executed (no interpreter).
    std::fs::write(&fixture.proton, "#!/nonexistent/interpreter\n").unwrap();
    std::fs::set_permissions(
        &fixture.proton,
        std::os::unix::fs::PermissionsExt::from_mode(0o755),
    )
    .unwrap();

    let error = fixture
        .service
        .start(JobOwner::new("main"), fixture.request())
        .await
        .err()
        .expect("spawn failure fails the start");
    let error = match error {
        RunExecutableStartError::Failed(error) => error,
        other => panic!("expected Failed(Io), got {other:?}"),
    };
    assert_eq!(error.kind(), AppErrorKind::Io);
    assert!(
        error
            .message()
            .starts_with("Failed to launch the executable"),
        "unexpected text: {}",
        error.message()
    );
    assert!(
        fixture.throwaway_dirs().is_empty(),
        "throwaway prefix removed"
    );
    assert!(fixture.log_files().is_empty(), "unused log removed");
    assert_eq!(fixture.jobs.job_counts(), (0, 0));
    assert_eq!(fixture.service.active.len(), 0);
}
