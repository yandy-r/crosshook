use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

use tokio::task;

use crate::platform::host_std_command;

const GAMESCOPE_STARTUP_POLL_ITERATIONS: u32 = 60;
const GAMESCOPE_POLL_INTERVAL: Duration = Duration::from_secs(2);
const GAMESCOPE_NATURAL_EXIT_GRACE: Duration = Duration::from_secs(5);
const GAMESCOPE_SIGTERM_WAIT: Duration = Duration::from_secs(3);
const GAMESCOPE_DESCENDANT_CLEANUP_DELAY: Duration = Duration::from_millis(500);
const TASK_COMM_LEN: usize = 15;

/// Checks whether a process whose name matches `exe_name` is currently running.
///
/// Scans `/proc/<pid>/comm` for exact matches, handling both the original name
/// (e.g. `game.exe`) and the name without the `.exe` suffix (`game`).
/// When `comm` is exactly 15 characters (the Linux `TASK_COMM_LEN` truncation
/// boundary), falls back to `/proc/<pid>/cmdline` for the full argv\[0\] basename.
pub fn is_process_running(exe_name: &str) -> bool {
    let candidates = process_name_candidates(exe_name);
    if candidates.is_empty() {
        return false;
    }

    let Ok(proc_dir) = fs::read_dir("/proc") else {
        return false;
    };

    for entry in proc_dir.flatten() {
        let dir_name = entry.file_name();
        let dir_name_str = dir_name.to_string_lossy();

        if dir_name_str.is_empty() || !dir_name_str.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }

        if process_path_matches_candidates(&entry.path(), &candidates) {
            return true;
        }
    }

    false
}

/// When gamescope wraps a Proton launch it becomes the direct child of
/// CrossHook but does **not** exit when the game inside it exits — lingering
/// clients (`mangoapp`, `winedevice.exe`, `gamescopereaper`) keep the
/// compositor alive indefinitely. This watchdog polls for the game executable
/// and, once it disappears, terminates gamescope so the normal
/// stream-log / finalize cleanup path can proceed.
pub async fn gamescope_watchdog(gamescope_pid: u32, exe_name: &str, killed_flag: Arc<AtomicBool>) {
    let mut game_seen = false;
    for _ in 0..GAMESCOPE_STARTUP_POLL_ITERATIONS {
        if !is_pid_alive(gamescope_pid) {
            return;
        }
        if descendant_process_running(gamescope_pid, exe_name.to_string()).await {
            game_seen = true;
            break;
        }
        tokio::time::sleep(GAMESCOPE_POLL_INTERVAL).await;
    }
    if !game_seen {
        tracing::debug!(
            exe = %exe_name,
            "gamescope watchdog: game process never appeared, standing down"
        );
        return;
    }

    loop {
        if !is_pid_alive(gamescope_pid) {
            return;
        }
        if !descendant_process_running(gamescope_pid, exe_name.to_string()).await {
            break;
        }
        tokio::time::sleep(GAMESCOPE_POLL_INTERVAL).await;
    }

    tracing::info!(
        exe = %exe_name,
        gamescope_pid,
        "gamescope watchdog: game exited, waiting for natural compositor shutdown"
    );

    tokio::time::sleep(GAMESCOPE_NATURAL_EXIT_GRACE).await;
    if !is_pid_alive(gamescope_pid) {
        return;
    }

    let descendants = collect_descendant_pids_task(gamescope_pid).await;

    killed_flag.store(true, Ordering::Release);
    tracing::info!(gamescope_pid, "gamescope watchdog: sending SIGTERM");
    signal_pid_task(gamescope_pid, None).await;

    tokio::time::sleep(GAMESCOPE_SIGTERM_WAIT).await;
    if !is_pid_alive(gamescope_pid) {
        kill_remaining_descendants_task(descendants).await;
        return;
    }

    tracing::warn!(gamescope_pid, "gamescope watchdog: sending SIGKILL");
    signal_pid_task(gamescope_pid, Some("-KILL")).await;

    tokio::time::sleep(GAMESCOPE_DESCENDANT_CLEANUP_DELAY).await;
    kill_remaining_descendants_task(descendants).await;
}

async fn descendant_process_running(gamescope_pid: u32, exe_name: String) -> bool {
    let exe_for_log = exe_name.clone();
    task::spawn_blocking(move || is_descendant_process_running(gamescope_pid, &exe_name))
        .await
        .unwrap_or_else(|error| {
            tracing::warn!(
                %error,
                gamescope_pid,
                exe = %exe_for_log,
                "gamescope watchdog: descendant scan task failed"
            );
            false
        })
}

async fn collect_descendant_pids_task(gamescope_pid: u32) -> Vec<u32> {
    task::spawn_blocking(move || collect_descendant_pids(gamescope_pid))
        .await
        .unwrap_or_else(|error| {
            tracing::warn!(
                %error,
                gamescope_pid,
                "gamescope watchdog: descendant collection task failed"
            );
            Vec::new()
        })
}

async fn kill_remaining_descendants_task(pids: Vec<u32>) {
    if pids.is_empty() {
        return;
    }

    let descendant_count = pids.len();
    if let Err(error) = task::spawn_blocking(move || kill_remaining_descendants(&pids)).await {
        tracing::warn!(
            %error,
            descendant_count,
            "gamescope watchdog: descendant cleanup task failed"
        );
    }
}

async fn signal_pid_task(pid: u32, signal: Option<&'static str>) {
    let result = task::spawn_blocking(move || {
        let mut command = host_std_command("kill");
        if let Some(signal) = signal {
            command.arg(signal);
        }
        command.arg(pid.to_string()).status()
    })
    .await;

    match result {
        Ok(Ok(_)) => {}
        Ok(Err(error)) => {
            tracing::warn!(%error, pid, ?signal, "gamescope watchdog: failed to signal process");
        }
        Err(error) => {
            tracing::warn!(
                %error,
                pid,
                ?signal,
                "gamescope watchdog: signal task join failed"
            );
        }
    }
}

fn is_descendant_process_running(root_pid: u32, exe_name: &str) -> bool {
    let candidates = process_name_candidates(exe_name);
    if candidates.is_empty() {
        return false;
    }

    collect_descendant_pids(root_pid)
        .into_iter()
        .any(|pid| process_matches_candidates(pid, &candidates))
}

fn process_name_candidates(exe_name: &str) -> Vec<String> {
    let name = exe_name.trim();
    if name.is_empty() {
        return Vec::new();
    }

    let mut candidates = vec![name.to_string()];
    if let Some(stripped) = name
        .strip_suffix(".exe")
        .or_else(|| name.strip_suffix(".EXE"))
    {
        candidates.push(stripped.to_string());
    }
    candidates
}

fn process_matches_candidates(pid: u32, candidates: &[String]) -> bool {
    let pid_path = format!("/proc/{pid}");
    process_path_matches_candidates(Path::new(&pid_path), candidates)
}

fn process_path_matches_candidates(pid_path: &Path, candidates: &[String]) -> bool {
    let Ok(comm) = fs::read_to_string(pid_path.join("comm")) else {
        return false;
    };
    let comm = comm.trim_end_matches('\n');
    if candidates.iter().any(|candidate| candidate == comm) {
        return true;
    }

    if comm.len() != TASK_COMM_LEN {
        return false;
    }

    let Ok(cmdline) = fs::read_to_string(pid_path.join("cmdline")) else {
        return false;
    };
    comm_matches_candidates(comm, Some(cmdline.as_str()), candidates)
}

fn comm_matches_candidates(comm: &str, cmdline: Option<&str>, candidates: &[String]) -> bool {
    if candidates.iter().any(|candidate| candidate == comm) {
        return true;
    }

    if comm.len() != TASK_COMM_LEN {
        return false;
    }

    let Some(cmdline) = cmdline else {
        return false;
    };
    let argv0 = cmdline.split('\0').next().unwrap_or("");
    let basename = argv0.rsplit('/').next().unwrap_or(argv0);
    let basename = basename.rsplit('\\').next().unwrap_or(basename);
    candidates.iter().any(|candidate| candidate == basename)
}

fn is_pid_alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

fn parse_status_ppid(status: &str) -> Option<u32> {
    status
        .lines()
        .find_map(|line| line.strip_prefix("PPid:\t"))
        .and_then(|ppid| ppid.trim().parse::<u32>().ok())
}

fn collect_descendant_pids(root_pid: u32) -> Vec<u32> {
    let mut children_map: HashMap<u32, Vec<u32>> = HashMap::new();
    let Ok(proc_dir) = fs::read_dir("/proc") else {
        return Vec::new();
    };

    for entry in proc_dir.flatten() {
        let name_os = entry.file_name();
        let name = name_os.to_string_lossy();
        if name.is_empty() || !name.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let Ok(pid) = name.parse::<u32>() else {
            continue;
        };
        let status_path = entry.path().join("status");
        let Ok(status) = fs::read_to_string(&status_path) else {
            continue;
        };
        if let Some(ppid) = parse_status_ppid(&status) {
            children_map.entry(ppid).or_default().push(pid);
        }
    }

    collect_descendant_pids_from_children_map(root_pid, &children_map)
}

fn collect_descendant_pids_from_children_map(
    root_pid: u32,
    children_map: &HashMap<u32, Vec<u32>>,
) -> Vec<u32> {
    let mut descendants = Vec::new();
    let mut queue = VecDeque::from([root_pid]);
    while let Some(parent) = queue.pop_front() {
        if let Some(children) = children_map.get(&parent) {
            for &child in children {
                descendants.push(child);
                queue.push_back(child);
            }
        }
    }
    descendants
}

fn kill_remaining_descendants(pids: &[u32]) {
    for &pid in pids {
        if is_pid_alive(pid) {
            tracing::info!(pid, "gamescope watchdog: killing orphaned descendant");
            // Accepted low-risk TOCTOU window: the pid could exit and be reused
            // between the liveness check and the signal delivery, but this cleanup
            // runs immediately after killing a same-UID gamescope tree on a
            // desktop system where reuse rates are low.
            let _ = host_std_command("kill")
                .arg("-KILL")
                .arg(pid.to_string())
                .status();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{
        collect_descendant_pids_from_children_map, comm_matches_candidates, parse_status_ppid,
        process_name_candidates,
    };

    #[test]
    fn parse_status_ppid_reads_expected_parent_pid() {
        let status = "Name:\tgamescope\nState:\tS (sleeping)\nPPid:\t42\n";

        assert_eq!(parse_status_ppid(status), Some(42));
    }

    #[test]
    fn descendant_collection_walks_entire_tree() {
        let children_map = HashMap::from([
            (10, vec![11, 12]),
            (11, vec![13]),
            (12, vec![14]),
            (13, vec![15]),
        ]);

        assert_eq!(
            collect_descendant_pids_from_children_map(10, &children_map),
            vec![11, 12, 13, 14, 15]
        );
    }

    #[test]
    fn truncated_task_comm_falls_back_to_cmdline_basename() {
        let candidates = process_name_candidates("1234567890123456.exe");

        assert!(comm_matches_candidates(
            "123456789012345",
            Some("/games/1234567890123456.exe\0--fullscreen"),
            &candidates
        ));
    }
}
