//! The `install-game` job: success with discovery, non-zero exit, prefix
//! exclusivity and shutdown cancellation.

use crosshook_core::install::InstallGameResult;
use tokio_util::sync::CancellationToken;

use super::Fixture;
use crate::jobs::JobRegistry;
use crate::log_tail::LogBatch;
use crate::services::process::{ProcessRun, RunEvent, MSG_PREFIX_BUSY};
use crate::{AppError, AppErrorKind};

type Completion = Result<InstallGameResult, AppError>;

/// Collects log lines until `Finished`; returns them with the completion.
async fn drain(run: &mut ProcessRun<Completion>) -> (Vec<String>, Completion) {
    let mut lines = Vec::new();
    loop {
        match run.next().await.expect("Finished before the stream ends") {
            RunEvent::Log(LogBatch::Lines(batch)) => lines.extend(batch),
            RunEvent::Log(_) => {}
            RunEvent::Finished(completion) => {
                return (lines, completion.expect("the job sent a completion"));
            }
        }
    }
}

/// Waits for log line `expected`; panics when the run finishes first.
async fn wait_for_line(run: &mut ProcessRun<Completion>, expected: &str) {
    loop {
        match run.next().await.expect("run ended early") {
            RunEvent::Log(LogBatch::Lines(batch)) if batch.iter().any(|l| l == expected) => {
                return;
            }
            RunEvent::Log(_) => {}
            RunEvent::Finished(_) => panic!("run finished before {expected:?}"),
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn successful_installer_completes_with_the_discovered_candidate() {
    // The runner checks it got the provisioned prefix, logs, and lays down
    // the game executable inside the prefix.
    let fixture = Fixture::new(
        "[ \"$WINEPREFIX\" = '@PREFIX@' ] || exit 12\n\
         [ -d '@PREFIX@' ] || exit 13\n\
         echo installing\n\
         mkdir -p \"$WINEPREFIX/drive_c/Games/X\"\n\
         echo game > \"$WINEPREFIX/drive_c/Games/X/X.exe\"\n\
         exit 0",
    );
    let jobs = JobRegistry::default();
    let service = fixture.service(jobs.clone());

    let mut run = service.start(fixture.request()).await.unwrap();
    let (lines, completion) = drain(&mut run).await;

    let result = completion.unwrap();
    assert!(lines.iter().any(|line| line == "installing"), "{lines:?}");
    assert!(result.succeeded);
    assert_eq!(
        result.message,
        "Installer completed. Review the generated profile."
    );
    assert_eq!(result.profile_name, "x-game");
    let expected = fixture
        .prefix
        .join("drive_c/Games/X/X.exe")
        .to_string_lossy()
        .into_owned();
    assert_eq!(result.discovered_game_executable_candidates, vec![expected]);
    let logs = fixture.log_files();
    assert_eq!(logs.len(), 1);
    assert_eq!(result.helper_log_path, logs[0].to_string_lossy());
    assert_eq!(std::fs::read_to_string(&logs[0]).unwrap(), "installing\n");
    assert_eq!(jobs.job_counts(), (0, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failing_installer_completes_with_the_status_code_text() {
    let fixture = Fixture::new("exit 3");
    let jobs = JobRegistry::default();
    let service = fixture.service(jobs.clone());

    let mut run = service.start(fixture.request()).await.unwrap();
    let (_, completion) = drain(&mut run).await;

    let error = completion.unwrap_err();
    assert_eq!(error.kind(), AppErrorKind::Io);
    assert!(error.message().contains("status code 3"), "{error}");
    assert_eq!(jobs.job_counts(), (0, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_request_fails_before_any_job_or_provisioning() {
    let fixture = Fixture::new("exit 0");
    let jobs = JobRegistry::default();
    let service = fixture.service(jobs.clone());
    let mut request = fixture.request();
    request.installer_path = fixture.dir.path().join("missing.exe").display().to_string();

    let error = service.start(request).await.err().unwrap();
    assert_eq!(
        error.kind(),
        AppErrorKind::Validation {
            field: "installer_path"
        }
    );
    assert!(!fixture.prefix.exists());
    assert!(fixture.log_files().is_empty());
    assert_eq!(jobs.job_counts(), (0, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn second_install_on_the_same_prefix_conflicts_while_the_first_runs() {
    let fixture = Fixture::new("echo waiting\nread x < '@GATE@'\nexit 0");
    let jobs = JobRegistry::default();
    let service = fixture.service(jobs.clone());

    let mut first = service.start(fixture.request()).await.unwrap();
    wait_for_line(&mut first, "waiting").await;
    let error = service.start(fixture.request()).await.err().unwrap();
    assert_eq!(error.kind(), AppErrorKind::Conflict);
    assert_eq!(error.message(), MSG_PREFIX_BUSY);
    // The rejected attempt left no log behind.
    assert_eq!(fixture.log_files().len(), 1);

    fixture.open_gate().await;
    let (_, completion) = drain(&mut first).await;
    assert!(completion.unwrap().succeeded);
    assert_eq!(jobs.job_counts(), (0, 0));

    // The key is released with the job.
    let mut again = service.start(fixture.request()).await.unwrap();
    fixture.open_gate().await;
    let (_, completion) = drain(&mut again).await;
    assert!(completion.unwrap().succeeded);
    assert_eq!(jobs.job_counts(), (0, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shutdown_cancels_the_installer() {
    let fixture = Fixture::new("echo waiting\nread x < '@GATE@'\nexit 0");
    let shutdown = CancellationToken::new();
    let jobs = JobRegistry::for_app(tokio::runtime::Handle::current(), &shutdown);
    let service = fixture.service(jobs.clone());

    let mut run = service.start(fixture.request()).await.unwrap();
    // The installer is blocked on the gate once its line arrived.
    wait_for_line(&mut run, "waiting").await;
    shutdown.cancel();
    let (_, completion) = drain(&mut run).await;

    let error = completion.unwrap_err();
    assert_eq!(error.kind(), AppErrorKind::Cancelled);
    assert_eq!(jobs.job_counts(), (0, 0));
}
