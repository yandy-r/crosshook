//! The dependency install job: offline gate, record-before-mutate (J4f),
//! spawn failure (J4g), live `in_progress`, restore after any install exit,
//! sanitized output and the raw request prefix in the wire payloads. Also
//! hosts the gate/drain helpers shared by the exclusivity and cancel tests.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;

use crosshook_core::prefix_deps::{DependencyState, PrefixVersionRestoreState};

use super::super::install::sanitize_batch;
use super::super::seams::MetadataJournalStore;
use super::super::{
    completion_payload, DepInstallCompletion, MSG_NO_TOOL_CHECK, MSG_NO_TOOL_INSTALL, MSG_OFFLINE,
};
use super::fakes::{FakeInUse, FakeProbe};
use super::helpers::{read_calls, Harness, ToolScript, PROFILE};
use crate::jobs::JobRegistry;
use crate::log_tail::LogBatch;
use crate::services::process::{ProcessRun, RunEvent};
use crate::AppErrorKind;

/// The package every test installs.
pub(super) const PACKAGE: &str = "dotnet48";

pub(super) fn packages() -> Vec<String> {
    vec![PACKAGE.to_string()]
}

/// The raw user-facing prefix of `h` (not the canonical `pfx`).
pub(super) fn raw_prefix(h: &Harness) -> String {
    h.prefix_root.to_string_lossy().into_owned()
}

/// Creates a FIFO at `<dir>/<name>`.
pub(super) fn make_fifo(dir: &Path, name: &str) -> PathBuf {
    let fifo = dir.join(name);
    let status = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .unwrap();
    assert!(status.success());
    fifo
}

/// Releases an install blocked reading `fifo`.
pub(super) async fn open_gate(fifo: PathBuf) {
    tokio::task::spawn_blocking(move || {
        let mut writer = OpenOptions::new().write(true).open(&fifo).unwrap();
        writer.write_all(b"go\n").unwrap();
    })
    .await
    .unwrap();
}

/// Waits until the run logged `expected`; panics when it finishes first.
pub(super) async fn wait_for_line(run: &mut ProcessRun<DepInstallCompletion>, expected: &str) {
    loop {
        match run.next().await.expect("run ended early") {
            RunEvent::Log(LogBatch::Lines(lines)) if lines.iter().any(|l| l == expected) => {
                return;
            }
            RunEvent::Log(_) => {}
            RunEvent::Finished(_) => panic!("run finished before {expected:?}"),
        }
    }
}

/// Collects the remaining log lines and the completion.
pub(super) async fn drain(
    run: &mut ProcessRun<DepInstallCompletion>,
) -> (Vec<String>, DepInstallCompletion) {
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

/// A gated install script and its fifo.
pub(super) fn gated_tool(h: &Harness) -> (String, PathBuf) {
    let fifo = make_fifo(h.dir.path(), "gate");
    let tool = h.tool(&ToolScript {
        install_gate: Some(fifo.clone()),
        ..ToolScript::default()
    });
    (tool, fifo)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn offline_mode_refuses_before_any_job() {
    let h = Harness::new();
    let tool = h.tool(&ToolScript::default());
    let mut settings = h.settings.load().unwrap();
    settings.offline_mode = true;
    h.settings.save(&settings).unwrap();
    let jobs = JobRegistry::default();
    let service = h.service(jobs.clone(), &tool);

    let error = service
        .start_install(PROFILE.to_string(), raw_prefix(&h), packages())
        .await
        .err()
        .expect("offline install must be refused");

    assert_eq!(error.kind(), AppErrorKind::Unavailable);
    assert_eq!(error.message(), MSG_OFFLINE);
    assert_eq!(jobs.job_counts(), (0, 0));
    assert!(read_calls(&h.calls).is_empty());
    assert!(h.row().is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn install_restores_after_success_and_streams_raw_prefix_payloads() {
    let h = Harness::new();
    let tool = h.tool(&ToolScript::default());
    let jobs = JobRegistry::default();
    let service = h.service(jobs.clone(), &tool);
    let raw = raw_prefix(&h);
    assert_ne!(
        raw, h.prefix,
        "the request path must not be the canonical pfx"
    );

    let mut run = service
        .start_install(PROFILE.to_string(), raw.clone(), packages())
        .await
        .unwrap();
    let (lines, completion) = drain(&mut run).await;

    assert!(
        lines.contains(&format!("installing {PACKAGE}")),
        "{lines:?}"
    );
    assert!(
        lines.contains(&format!("install warning {PACKAGE}")),
        "{lines:?}"
    );
    // Install first, then the restore of the captured (win7) version.
    assert_eq!(read_calls(&h.calls), vec!["-q dotnet48", "-q win7"]);
    let outcome = completion.outcome;
    assert!(outcome.install_succeeded);
    assert_eq!(outcome.install_exit_code, Some(0));
    assert_eq!(outcome.restore_state, PrefixVersionRestoreState::Succeeded);
    assert!(h.row().is_none(), "a verified restore deletes the row");
    assert_eq!(jobs.job_counts(), (0, 0));

    // The wire payloads and dep states use the raw request prefix
    // (listeners and `get_dependency_status` match it exactly).
    let payload = completion_payload(PROFILE, raw.clone(), outcome);
    assert!(payload.succeeded);
    assert_eq!(payload.prefix_path, raw);

    let canonical_states = service
        .dependency_status(PROFILE.to_string(), h.prefix.clone())
        .await
        .unwrap();
    assert!(
        canonical_states.is_empty(),
        "states are keyed on the raw path"
    );
    let states = service
        .dependency_status(PROFILE.to_string(), raw)
        .await
        .unwrap();
    assert_eq!(states.len(), 1);
    assert_eq!(states[0].package_name, PACKAGE);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_install_still_restores_and_reports_failure() {
    let h = Harness::new();
    let tool = h.tool(&ToolScript {
        install_exit: 3,
        ..ToolScript::default()
    });
    let service = h.service(JobRegistry::default(), &tool);

    let mut run = service
        .start_install(PROFILE.to_string(), raw_prefix(&h), packages())
        .await
        .unwrap();
    let (_, completion) = drain(&mut run).await;

    assert_eq!(read_calls(&h.calls), vec!["-q dotnet48", "-q win7"]);
    assert!(!completion.outcome.install_succeeded);
    assert_eq!(completion.outcome.install_exit_code, Some(3));
    assert_eq!(
        completion.outcome.restore_state,
        PrefixVersionRestoreState::Succeeded
    );
    assert!(h.row().is_none());
}

// J4f: a failed `pending` record means nothing is spawned (I1).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn j4f_record_failure_spawns_nothing() {
    let h = Harness::new();
    let tool = h.tool(&ToolScript::default());
    let store = h.recording_store();
    store.fail_upsert.store(true, Ordering::SeqCst);
    let restorer = h.restorer();
    let jobs = JobRegistry::default();
    let service = h.service_with(
        jobs.clone(),
        store.clone(),
        restorer.clone(),
        Arc::new(FakeInUse::new(false, &h.prefix)),
        Arc::new(FakeProbe::winetricks(&tool)),
    );

    let error = service
        .start_install(PROFILE.to_string(), raw_prefix(&h), packages())
        .await
        .err()
        .expect("a failed record must fail the install");

    // The store error surfaces sanitized (fixed text, no SQL or paths).
    assert_eq!(error.kind(), AppErrorKind::Validation { field: "metadata" });
    assert!(read_calls(&h.calls).is_empty(), "nothing may be spawned");
    assert_eq!(restorer.count(), 0);
    assert_eq!(store.ops(), vec!["load", "upsert"]);
    assert!(h.row().is_none());
    assert_eq!(jobs.job_counts(), (0, 0));
}

// J4g: a spawn failure after the record fails the start (C-3: no completion
// is sent for a child that never ran) and keeps the row `pending`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn j4g_spawn_failure_keeps_row_pending() {
    let h = Harness::new();
    let missing = h.dir.path().join("missing-tricks");
    let restorer = h.restorer();
    let service = h.service_with(
        JobRegistry::default(),
        Arc::new(MetadataJournalStore(h.metadata.clone())),
        restorer.clone(),
        Arc::new(FakeInUse::new(false, &h.prefix)),
        Arc::new(FakeProbe::winetricks(&missing.to_string_lossy())),
    );

    let error = service
        .start_install(PROFILE.to_string(), raw_prefix(&h), packages())
        .await
        .err()
        .expect("a tool that cannot be spawned fails the start");

    assert_eq!(error.kind(), AppErrorKind::Io);
    assert!(!error.message().contains("/home/"), "{}", error.message());
    assert_eq!(restorer.count(), 0, "no restore with a tool that never ran");
    let row = h
        .row()
        .expect("the pending row must survive a spawn failure");
    assert_eq!(row.state, "pending");
    let status = service.repair_status(raw_prefix(&h)).await.unwrap();
    assert!(status.required);
    assert_eq!(status.state, Some(PrefixVersionRestoreState::Pending));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unindexed_profile_is_refused_before_spawn() {
    let h = Harness::new();
    let tool = h.tool(&ToolScript::default());
    let jobs = JobRegistry::default();
    let service = h.service(jobs.clone(), &tool);

    let error = service
        .start_install("ghost".to_string(), raw_prefix(&h), packages())
        .await
        .err()
        .expect("an unindexed profile must be refused");

    assert_eq!(error.kind(), AppErrorKind::NotFound);
    assert!(
        error.message().contains("profile is not indexed"),
        "{error}"
    );
    assert!(read_calls(&h.calls).is_empty());
    assert!(h.row().is_none());
    assert_eq!(jobs.job_counts(), (0, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn status_reports_in_progress_while_installing() {
    let h = Harness::new();
    let (tool, fifo) = gated_tool(&h);
    let service = h.service(JobRegistry::default(), &tool);

    let mut run = service
        .start_install(PROFILE.to_string(), raw_prefix(&h), packages())
        .await
        .unwrap();
    wait_for_line(&mut run, "installing dotnet48").await;

    let during = service.repair_status(raw_prefix(&h)).await.unwrap();
    assert!(during.in_progress);
    assert!(during.required, "the pending row exists while installing");

    open_gate(fifo).await;
    let (_, completion) = drain(&mut run).await;
    assert_eq!(
        completion.outcome.restore_state,
        PrefixVersionRestoreState::Succeeded
    );
    let after = service.repair_status(raw_prefix(&h)).await.unwrap();
    assert!(!after.in_progress);
    assert!(!after.required);
}

#[test]
fn install_output_is_sanitized() {
    let batch = LogBatch::Lines(vec![
        "\u{1b}[31mfailed\u{1b}[0m at /home/someone/secret".to_string()
    ]);
    assert_eq!(
        sanitize_batch(batch),
        LogBatch::Lines(vec!["failed at <path>".to_string()])
    );
    assert_eq!(
        sanitize_batch(LogBatch::Dropped { n: 2 }),
        LogBatch::Dropped { n: 2 }
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn missing_tool_keeps_the_check_and_install_texts() {
    let h = Harness::new();
    let probe = Arc::new(FakeProbe::none());
    let jobs = JobRegistry::default();
    let service = h.service_with(
        jobs.clone(),
        Arc::new(MetadataJournalStore(h.metadata.clone())),
        h.restorer(),
        Arc::new(FakeInUse::new(false, &h.prefix)),
        probe.clone(),
    );

    let check = service
        .check(PROFILE.to_string(), raw_prefix(&h), packages())
        .await
        .expect_err("no tool, no check");
    assert_eq!(check.kind(), AppErrorKind::Unavailable);
    assert_eq!(check.message(), MSG_NO_TOOL_CHECK);
    let install = service
        .start_install(PROFILE.to_string(), raw_prefix(&h), packages())
        .await
        .err()
        .expect("no tool, no install");
    assert_eq!(install.kind(), AppErrorKind::Unavailable);
    assert_eq!(install.message(), MSG_NO_TOOL_INSTALL);
    // Both asked detection with the (empty) settings override path.
    assert_eq!(
        *probe.settings_paths.lock().unwrap(),
        vec![String::new(), String::new()]
    );
    assert_eq!(jobs.job_counts(), (0, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn check_lists_installed_packages_without_taking_the_key() {
    let h = Harness::new();
    let script = h.dir.path().join("list-tricks");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\n[ -n \"$WINEPREFIX\" ] || exit 9\necho 'dotnet48 vcrun2019'\n",
            h.calls.display()
        ),
    )
    .unwrap();
    super::helpers::set_mode(&script, 0o755);
    let jobs = JobRegistry::default();
    let service = h.service(jobs.clone(), &script.to_string_lossy());

    let statuses = service
        .check(
            PROFILE.to_string(),
            raw_prefix(&h),
            vec!["dotnet48".to_string(), "d3dx9".to_string()],
        )
        .await
        .unwrap();

    assert_eq!(read_calls(&h.calls), vec!["list-installed"]);
    let states: Vec<_> = statuses
        .iter()
        .map(|status| (status.package_name.as_str(), status.state.clone()))
        .collect();
    assert_eq!(
        states,
        vec![
            ("dotnet48", DependencyState::Installed),
            ("d3dx9", DependencyState::Missing),
        ]
    );
    assert_eq!(jobs.job_counts(), (0, 0), "the check runs no job");
    let cached = service
        .dependency_status(PROFILE.to_string(), raw_prefix(&h))
        .await
        .unwrap();
    assert_eq!(cached.len(), 2);
}
