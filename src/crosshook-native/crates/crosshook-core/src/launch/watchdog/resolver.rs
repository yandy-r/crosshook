use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use tokio::time;

use super::descendants::collect_descendant_pids_from_children_map;
use super::{
    capture::{
        match_captured_pid, read_pid_capture_file_blocking, CaptureOwner, CaptureRead,
        CapturedPidMatch,
    },
    host::{collect_host_process_entries, host_process_entry_matches_candidates, HostProcessEntry},
    process::{is_pid_alive, process_name_candidates},
    ShutdownTarget, GAMESCOPE_HOST_PID_CAPTURE_WAIT_INTERVAL,
    GAMESCOPE_HOST_PID_CAPTURE_WAIT_ITERATIONS,
};

pub(super) fn is_target_pid_alive(target: ShutdownTarget) -> bool {
    if target.host_namespace {
        super::host::is_host_pid_alive(target.pid)
    } else {
        is_pid_alive(target.pid)
    }
}

pub(super) fn resolve_watchdog_target_by_host_ancestor_search(
    exe_name: &str,
) -> (Option<ShutdownTarget>, Vec<u32>) {
    let entries = collect_host_process_entries();
    if entries.is_empty() {
        return (None, Vec::new());
    }

    let exe_candidates = process_name_candidates(exe_name);
    if exe_candidates.is_empty() {
        return (None, Vec::new());
    }

    let mut parent_by_pid = HashMap::new();
    let mut children_map: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut gamescope_pids = Vec::new();
    let mut matching_game_pids = Vec::new();

    for entry in &entries {
        parent_by_pid.insert(entry.pid, entry.ppid);
        children_map.entry(entry.ppid).or_default().push(entry.pid);

        if entry.comm == "gamescope" {
            gamescope_pids.push(entry.pid);
        }

        if host_process_entry_matches_candidates(entry, &exe_candidates) {
            matching_game_pids.push(entry.pid);
        }
    }

    let Some(gamescope_pid) =
        resolve_unique_gamescope_ancestor(&parent_by_pid, &matching_game_pids, &gamescope_pids)
    else {
        return (None, Vec::new());
    };

    let descendants = collect_descendant_pids_from_children_map(gamescope_pid, &children_map);
    (
        Some(ShutdownTarget {
            pid: gamescope_pid,
            host_namespace: true,
        }),
        descendants,
    )
}

/// How long to wait for the capture file and its gamescope process.
#[derive(Clone, Copy, Debug)]
pub(super) struct CaptureBudget {
    pub(super) iterations: u32,
    pub(super) interval: Duration,
}

impl CaptureBudget {
    pub(super) const DEFAULT: Self = Self {
        iterations: GAMESCOPE_HOST_PID_CAPTURE_WAIT_ITERATIONS,
        interval: GAMESCOPE_HOST_PID_CAPTURE_WAIT_INTERVAL,
    };
}

pub(super) async fn resolve_watchdog_target(
    observed_gamescope_pid: u32,
    host_pid_capture_path: Option<&Path>,
    exe_name: &str,
) -> Option<ShutdownTarget> {
    let Some(path) = host_pid_capture_path else {
        return Some(ShutdownTarget {
            pid: observed_gamescope_pid,
            host_namespace: false,
        });
    };

    let captured = await_captured_gamescope(
        path,
        CaptureBudget::DEFAULT,
        CaptureOwner::current(),
        collect_host_process_entries,
        exe_name,
        observed_gamescope_pid,
    )
    .await;
    if captured.is_some() {
        return captured;
    }
    resolve_by_host_ancestor_fallback(exe_name, observed_gamescope_pid).await
}

/// Polls the capture file and accepts its PID only once the host snapshot shows
/// it as a gamescope process of `owner`. `None` means "use the fallback".
pub(super) async fn await_captured_gamescope<F>(
    path: &Path,
    budget: CaptureBudget,
    owner: CaptureOwner,
    snapshot: F,
    exe_name: &str,
    observed_gamescope_pid: u32,
) -> Option<ShutdownTarget>
where
    F: Fn() -> Vec<HostProcessEntry> + Clone + Send + 'static,
{
    let mut captured_pid: Option<u32> = None;
    for _ in 0..budget.iterations {
        if captured_pid.is_none() {
            match read_pid_capture_file_blocking(path).await {
                CaptureRead::Pending => {}
                CaptureRead::Rejected => {
                    tracing::warn!(
                        reason = "invalid_pid",
                        game_exe = %exe_name,
                        observed_gamescope_pid,
                        "gamescope watchdog: rejected PID capture file"
                    );
                    return None;
                }
                CaptureRead::Pid(pid) => captured_pid = Some(pid),
            }
        }

        if let Some(pid) = captured_pid {
            let entries = tokio::task::spawn_blocking(snapshot.clone())
                .await
                .unwrap_or_default();
            match match_captured_pid(&entries, pid, owner) {
                CapturedPidMatch::NotYet => {}
                CapturedPidMatch::UidUnknown => {
                    tracing::warn!(
                        reason = "uid_unknown",
                        discovered_pid = pid,
                        game_exe = %exe_name,
                        observed_gamescope_pid,
                        "gamescope watchdog: host process table has no uids; cannot verify the captured PID"
                    );
                    return None;
                }
                CapturedPidMatch::Owned => {
                    tracing::info!(
                        fallback = "capture_file",
                        discovered_pid = pid,
                        game_exe = %exe_name,
                        observed_gamescope_pid,
                        "gamescope watchdog: resolved target via capture file"
                    );
                    return Some(ShutdownTarget {
                        pid,
                        host_namespace: true,
                    });
                }
            }
        }
        time::sleep(budget.interval).await;
    }

    if let Some(pid) = captured_pid {
        tracing::warn!(
            reason = "not_gamescope",
            discovered_pid = pid,
            game_exe = %exe_name,
            observed_gamescope_pid,
            "gamescope watchdog: captured PID never matched a gamescope process"
        );
    }
    None
}

async fn resolve_by_host_ancestor_fallback(
    exe_name: &str,
    observed_gamescope_pid: u32,
) -> Option<ShutdownTarget> {
    let search_exe = exe_name.to_string();
    let (exe_target, descendants) = tokio::task::spawn_blocking(move || {
        resolve_watchdog_target_by_host_ancestor_search(&search_exe)
    })
    .await
    .unwrap_or((None, Vec::new()));

    if let Some(target) = exe_target {
        tracing::info!(
            fallback = "exe_fallback",
            discovered_pid = target.pid,
            game_exe = %exe_name,
            observed_descendants = descendants.len(),
            observed_gamescope_pid,
            "gamescope watchdog: resolved target via host ancestor match"
        );
        return Some(target);
    }

    tracing::warn!(
        fallback = "none",
        game_exe = %exe_name,
        observed_descendants = descendants.len(),
        observed_gamescope_pid,
        "gamescope watchdog: no target resolved; standing down"
    );
    None
}

pub(super) fn resolve_unique_gamescope_ancestor(
    parent_by_pid: &HashMap<u32, u32>,
    matching_game_pids: &[u32],
    gamescope_pids: &[u32],
) -> Option<u32> {
    let mut unique_gamescope_pid = None;
    let mut matched_any_ancestor = false;
    for &game_pid in matching_game_pids {
        let Some(gamescope_pid) = find_matching_ancestor_pid(game_pid, parent_by_pid, |pid| {
            gamescope_pids.contains(&pid)
        }) else {
            continue;
        };
        matched_any_ancestor = true;
        match unique_gamescope_pid {
            Some(existing) if existing != gamescope_pid => return None,
            Some(_) => {}
            None => unique_gamescope_pid = Some(gamescope_pid),
        }
    }
    if matched_any_ancestor {
        unique_gamescope_pid
    } else {
        None
    }
}

fn find_matching_ancestor_pid(
    start_pid: u32,
    parent_by_pid: &HashMap<u32, u32>,
    matches_pid: impl Fn(u32) -> bool,
) -> Option<u32> {
    let mut current = parent_by_pid.get(&start_pid).copied();
    while let Some(pid) = current {
        if matches_pid(pid) {
            return Some(pid);
        }
        let next = parent_by_pid.get(&pid).copied();
        if next == Some(pid) {
            break;
        }
        current = next;
    }
    None
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    const OWNER: CaptureOwner = CaptureOwner { uid: 1000 };
    const BUDGET: CaptureBudget = CaptureBudget {
        iterations: 3,
        interval: Duration::from_millis(1),
    };

    fn gamescope(pid: u32, uid: u32) -> Vec<HostProcessEntry> {
        vec![HostProcessEntry {
            pid,
            ppid: 1,
            uid: Some(uid),
            comm: "gamescope".to_string(),
        }]
    }

    async fn await_with(
        path: &Path,
        snapshot: impl Fn() -> Vec<HostProcessEntry> + Clone + Send + 'static,
    ) -> Option<ShutdownTarget> {
        await_captured_gamescope(path, BUDGET, OWNER, snapshot, "game.exe", 7).await
    }

    #[tokio::test]
    async fn missing_file_until_budget_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let target = await_with(&dir.path().join("pid"), || gamescope(4242, 1000)).await;
        assert_eq!(target, None);
    }

    #[tokio::test]
    async fn rejected_file_returns_none_and_is_removed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pid");
        fs::write(&path, "not-a-pid").unwrap();
        assert_eq!(await_with(&path, || gamescope(4242, 1000)).await, None);
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn pid_that_is_not_gamescope_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pid");
        fs::write(&path, "4242").unwrap();
        assert_eq!(await_with(&path, || gamescope(9999, 1000)).await, None);
    }

    #[tokio::test]
    async fn gamescope_of_another_user_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pid");
        fs::write(&path, "4242").unwrap();
        assert_eq!(await_with(&path, || gamescope(4242, 1001)).await, None);
    }

    #[tokio::test]
    async fn snapshot_without_uids_stops_early() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pid");
        fs::write(&path, "4242").unwrap();
        let budget = CaptureBudget {
            iterations: 1_000,
            interval: Duration::from_secs(1),
        };
        let started = std::time::Instant::now();
        let target = await_captured_gamescope(
            &path,
            budget,
            OWNER,
            || {
                vec![HostProcessEntry {
                    pid: 4242,
                    ppid: 1,
                    uid: None,
                    comm: "gamescope".to_string(),
                }]
            },
            "game.exe",
            7,
        )
        .await;
        assert_eq!(target, None);
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[tokio::test]
    async fn owned_gamescope_pid_becomes_host_target() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pid");
        fs::write(&path, "4242\n").unwrap();
        assert_eq!(
            await_with(&path, || gamescope(4242, 1000)).await,
            Some(ShutdownTarget {
                pid: 4242,
                host_namespace: true,
            })
        );
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn empty_file_keeps_polling_until_the_pid_is_written() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pid");
        fs::write(&path, "").unwrap();
        let writer_path = path.clone();
        let budget = CaptureBudget {
            iterations: 200,
            interval: Duration::from_millis(5),
        };
        let writer = tokio::spawn(async move {
            time::sleep(Duration::from_millis(20)).await;
            fs::write(&writer_path, "4242").unwrap();
        });
        let target = await_captured_gamescope(
            &path,
            budget,
            OWNER,
            || gamescope(4242, 1000),
            "game.exe",
            7,
        )
        .await;
        writer.await.unwrap();
        assert_eq!(target.map(|target| target.pid), Some(4242));
    }
}
