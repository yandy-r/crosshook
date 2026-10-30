//! The production in-use probe (S-1): a process using the prefix through a
//! symlinked alias is found; under Flatpak the probe fails closed.

use std::path::Path;
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

use super::super::seams::prefix_in_use_with;
use super::helpers::Harness;

/// Spawns `env <var>=<value> sh` that blocks on `read` after printing `ready`.
async fn spawn_with_env(var: &str, value: &Path) -> Child {
    let mut child = Command::new("env")
        .arg(format!("{var}={}", value.display()))
        .arg("sh")
        .arg("-c")
        .arg("echo ready; read line")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn env child");
    let mut stdout = BufReader::new(child.stdout.take().expect("piped stdout"));
    let mut line = String::new();
    stdout.read_line(&mut line).await.expect("read ready line");
    assert_eq!(line.trim(), "ready");
    child
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prefix_used_through_a_symlinked_alias_is_in_use() {
    let h = Harness::new();
    assert!(!prefix_in_use_with(h.pfx(), false), "nobody uses it yet");
    let alias_root = h.dir.path().join("alias");
    std::os::unix::fs::symlink(&h.prefix_root, &alias_root).unwrap();

    let _game = spawn_with_env("STEAM_COMPAT_DATA_PATH", &alias_root).await;

    assert!(prefix_in_use_with(h.pfx(), false));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unrelated_variable_naming_the_prefix_is_not_use() {
    let h = Harness::new();

    let _shell = spawn_with_env("HOME", h.pfx()).await;

    assert!(!prefix_in_use_with(h.pfx(), false));
}

#[test]
fn flatpak_fails_closed() {
    let h = Harness::new();
    assert!(prefix_in_use_with(h.pfx(), true));
}
