//! `spawn_pipe_tee`: the append open never creates the log and never follows
//! a symlink planted at its path.

use std::os::unix::fs::symlink;
use std::process::Stdio;

use tokio::process::Command;

use crate::services::process::tee::spawn_pipe_tee;

async fn tee_echo(log_path: &std::path::Path) {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg("echo teed")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let tee = spawn_pipe_tee(&mut child, log_path).await;
    child.wait().await.unwrap();
    tee.finish().await;
}

#[tokio::test]
async fn appends_to_an_existing_regular_log() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("run.log");
    std::fs::write(&log, "before\n").unwrap();

    tee_echo(&log).await;

    assert_eq!(std::fs::read_to_string(&log).unwrap(), "before\nteed\n");
}

#[tokio::test]
async fn a_symlink_at_the_log_path_is_refused_and_its_target_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("victim");
    std::fs::write(&target, "original").unwrap();
    let log = dir.path().join("run.log");
    symlink(&target, &log).unwrap();

    tee_echo(&log).await;

    assert_eq!(std::fs::read_to_string(&target).unwrap(), "original");
}

#[tokio::test]
async fn a_missing_log_is_not_created() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("run.log");

    tee_echo(&log).await;

    assert!(!log.exists());
}
