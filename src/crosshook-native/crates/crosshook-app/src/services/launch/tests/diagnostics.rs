//! Diagnostic helper and bounded log tail tests (Task 2.2).

use crosshook_core::launch::{METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH};

use super::super::diagnostics::{diagnostic_method_for_log, read_log_tail};

#[test]
fn diagnostic_method_uses_proton_run_for_trainer_runner_logs() {
    let log_tail = "[steam-helper] Delegating trainer leg to steam-host-trainer-runner.sh\n[steam-trainer-runner] trainer_launch_mode=direct_proton\n";

    assert_eq!(
        diagnostic_method_for_log(METHOD_STEAM_APPLAUNCH, log_tail),
        METHOD_PROTON_RUN
    );
}

#[test]
fn diagnostic_method_keeps_steam_for_plain_helper_logs() {
    let log_tail = "[steam-helper] Launching Steam AppID 12345\n";

    assert_eq!(
        diagnostic_method_for_log(METHOD_STEAM_APPLAUNCH, log_tail),
        METHOD_STEAM_APPLAUNCH
    );
}

#[tokio::test]
async fn read_log_tail_returns_only_the_last_cap_bytes_of_a_larger_log() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("big.log");
    let body = format!("HEAD{}TAIL", "x".repeat(100));
    std::fs::write(&path, &body).unwrap();

    let tail = read_log_tail(&path, 10).await;

    assert_eq!(tail, "xxxxxxTAIL");
    assert!(tail.len() <= 10);
    assert!(!tail.contains("HEAD"));
}

#[tokio::test]
async fn read_log_tail_returns_whole_file_when_under_cap() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("small.log");
    std::fs::write(&path, "line one\nline two\n").unwrap();

    assert_eq!(read_log_tail(&path, 1024).await, "line one\nline two\n");
}

#[tokio::test]
async fn read_log_tail_is_lossy_when_the_seek_splits_a_character() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("split.log");
    // Ten two-byte characters: 20 bytes. A 5-byte tail starts on a
    // continuation byte, which must decode to U+FFFD rather than fail.
    std::fs::write(&path, "é".repeat(10)).unwrap();

    let tail = read_log_tail(&path, 5).await;

    assert!(tail.starts_with('\u{FFFD}'));
    assert!(tail.ends_with("éé"));
}

#[tokio::test]
async fn read_log_tail_decodes_invalid_utf8_lossily() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.log");
    std::fs::write(&path, b"ok\xff\xfeend").unwrap();

    assert_eq!(read_log_tail(&path, 1024).await, "ok\u{FFFD}\u{FFFD}end");
}

#[tokio::test]
async fn read_log_tail_returns_empty_for_a_missing_file() {
    let dir = tempfile::tempdir().unwrap();

    assert_eq!(read_log_tail(&dir.path().join("missing.log"), 1024).await, "");
}
