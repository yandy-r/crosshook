use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
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
const GAMESCOPE_HOST_PID_CAPTURE_WAIT_ITERATIONS: u32 = 20;
const GAMESCOPE_HOST_PID_CAPTURE_WAIT_INTERVAL: Duration = Duration::from_millis(200);
const TASK_COMM_LEN: usize = 15;

#[derive(Debug, Default)]
struct HostProcessProbe {
    visible_count: usize,
    exact_matches: Vec<String>,
    truncated_matches: Vec<String>,
}

impl HostProcessProbe {
    fn has_match(&self) -> bool {
        !self.exact_matches.is_empty() || !self.truncated_matches.is_empty()
    }
}

#[derive(Clone, Copy, Debug)]
struct ShutdownTarget {
    pid: u32,
    host_namespace: bool,
}

fn read_pid_capture_file(path: &Path) -> Option<u32> {
    let content = fs::read_to_string(path).ok().or_else(|| {
        if crate::platform::is_flatpak() {
            read_host_text_file(path)
        } else {
            None
        }
    })?;
    content.trim().parse::<u32>().ok().filter(|pid| *pid > 0)
}

fn read_host_text_file(path: &Path) -> Option<String> {
    let output = host_std_command("cat").arg(path).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn is_host_pid_alive(pid: u32) -> bool {
    host_std_command("test")
        .arg("-d")
        .arg(format!("/proc/{pid}"))
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn is_target_pid_alive(target: ShutdownTarget) -> bool {
    if target.host_namespace {
        is_host_pid_alive(target.pid)
    } else {
        is_pid_alive(target.pid)
    }
}

fn parse_host_pid_and_ppid(line: &str) -> Option<(u32, u32)> {
    let mut fields = line.split('\t');
    let pid = fields.next()?.trim().parse().ok()?;
    let ppid = fields.next()?.trim().parse().ok()?;
    Some((pid, ppid))
}

fn collect_host_descendant_pids(root_pid: u32) -> Vec<u32> {
    let output = match host_std_command("ps")
        .args(["-eo", "pid=", "-o", "ppid=", "--delimiter", "\t"])
        .output()
    {
        Ok(output) if output.status.success() => output,
        _ => return Vec::new(),
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut children_map: HashMap<u32, Vec<u32>> = HashMap::new();
    for line in stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if let Some((pid, ppid)) = parse_host_pid_and_ppid(line) {
            children_map.entry(ppid).or_default().push(pid);
        }
    }
    collect_descendant_pids_from_children_map(root_pid, &children_map)
}

/// Walks the host-ps tree from `root_pid` and searches descendants for a
/// process whose `comm`/`cmdline` matches `exe_name`.
///
/// Returns `(Some(ShutdownTarget), descendants)` when a match is found, or
/// `(None, descendants)` otherwise. The caller always receives the full
/// descendant list so it can emit `observed_descendants` (count) together with
/// `observed_gamescope_pid` regardless of outcome.
fn resolve_watchdog_target_by_exe_name(
    root_pid: u32,
    exe_name: &str,
) -> (Option<ShutdownTarget>, Vec<u32>) {
    let candidates = process_name_candidates(exe_name);
    let descendants = collect_host_descendant_pids(root_pid);
    let target = descendants
        .iter()
        .copied()
        .find(|&pid| host_process_matches_candidates(pid, &candidates))
        .map(|pid| ShutdownTarget {
            pid,
            host_namespace: true,
        });
    (target, descendants)
}

async fn resolve_watchdog_target(
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

    for _ in 0..GAMESCOPE_HOST_PID_CAPTURE_WAIT_ITERATIONS {
        if let Some(pid) = read_pid_capture_file(path) {
            let target = ShutdownTarget {
                pid,
                host_namespace: true,
            };
            tracing::info!(
                fallback = "capture_file",
                discovered_pid = target.pid,
                game_exe = %exe_name,
                observed_gamescope_pid,
                "gamescope watchdog: resolved target via capture file"
            );
            return Some(target);
        }
        tokio::time::sleep(GAMESCOPE_HOST_PID_CAPTURE_WAIT_INTERVAL).await;
    }

    let (exe_target, descendants) =
        resolve_watchdog_target_by_exe_name(observed_gamescope_pid, exe_name);

    if let Some(target) = exe_target {
        tracing::info!(
            fallback = "exe_fallback",
            discovered_pid = target.pid,
            game_exe = %exe_name,
            observed_descendants = descendants.len(),
            observed_gamescope_pid,
            "gamescope watchdog: resolved target via exe-name descendant match"
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

fn host_process_line_parts(line: &str) -> Option<(u32, &str)> {
    let (pid, comm) = line.split_once('\t')?;
    let pid = pid.trim().parse().ok()?;
    let comm = comm.trim();
    if comm.is_empty() {
        return None;
    }
    Some((pid, comm))
}

fn read_host_process_cmdline(pid: u32) -> Option<String> {
    let output = host_std_command("cat")
        .arg(format!("/proc/{pid}/cmdline"))
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn probe_host_processes(candidates: &[String]) -> HostProcessProbe {
    let output = match host_std_command("ps")
        .args(["-ww", "-eo", "pid=", "-o", "comm=", "--delimiter", "\t"])
        .output()
    {
        Ok(output) => output,
        Err(_) => return HostProcessProbe::default(),
    };
    if !output.status.success() {
        return HostProcessProbe::default();
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut probe = HostProcessProbe::default();
    for line in stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        probe.visible_count += 1;
        let Some((pid, comm)) = host_process_line_parts(line) else {
            continue;
        };

        if probe.exact_matches.len() < 3 && candidates.iter().any(|candidate| candidate == comm) {
            probe.exact_matches.push(comm.to_string());
            continue;
        }

        if probe.truncated_matches.len() < 3 && comm.len() == TASK_COMM_LEN {
            let cmdline = read_host_process_cmdline(pid);
            if !comm_matches_candidates(comm, cmdline.as_deref(), candidates) {
                continue;
            }
            let argv0 = cmdline
                .as_deref()
                .and_then(|cmdline| cmdline.split('\0').next())
                .unwrap_or("");
            let basename = argv0.rsplit('/').next().unwrap_or(argv0);
            let basename = basename.rsplit('\\').next().unwrap_or(basename);
            probe.truncated_matches.push(basename.to_string());
        }
    }
    probe
}

/// Checks whether a process whose name matches `exe_name` is currently running.
///
/// Scans `/proc/<pid>/comm` for exact matches, handling both the original name
/// (e.g. `game.exe`) and the name without the `.exe` suffix (`game`).
/// When `comm` is exactly 15 characters (the Linux `TASK_COMM_LEN` truncation
/// boundary), falls back to `/proc/<pid>/cmdline` for the full argv\[0\] basename.
/// Under Flatpak, prefers the host `ps` view when available because the sandbox
/// PID namespace may hide the real game process.
pub fn is_process_running(exe_name: &str) -> bool {
    let candidates = process_name_candidates(exe_name);
    if candidates.is_empty() {
        return false;
    }

    let is_flatpak = crate::platform::is_flatpak();
    let mut sandbox_match = false;
    if let Ok(proc_dir) = fs::read_dir("/proc") {
        for entry in proc_dir.flatten() {
            let dir_name = entry.file_name();
            let dir_name_str = dir_name.to_string_lossy();

            if dir_name_str.is_empty() || !dir_name_str.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }

            if process_path_matches_candidates(&entry.path(), &candidates) {
                sandbox_match = true;
                break;
            }
        }
    }

    let host_probe = if is_flatpak {
        Some(probe_host_processes(&candidates))
    } else {
        None
    };
    let running = sandbox_match || host_probe.as_ref().is_some_and(HostProcessProbe::has_match);

    running
}

/// When gamescope wraps a Proton launch it becomes the direct child of
/// CrossHook but does **not** exit when the game inside it exits — lingering
/// clients (`mangoapp`, `winedevice.exe`, `gamescopereaper`) keep the
/// compositor alive indefinitely. This watchdog polls for the game executable
/// and, once it disappears, terminates gamescope so the normal
/// stream-log / finalize cleanup path can proceed.
pub async fn gamescope_watchdog(
    gamescope_pid: u32,
    exe_name: &str,
    killed_flag: Arc<AtomicBool>,
    host_pid_capture_path: Option<PathBuf>,
) {
    let Some(observation_target) =
        resolve_watchdog_target(gamescope_pid, host_pid_capture_path.as_deref(), exe_name).await
    else {
        return;
    };
    let mut game_seen = false;
    for _ in 0..GAMESCOPE_STARTUP_POLL_ITERATIONS {
        if !is_target_pid_alive(observation_target) {
            return;
        }
        if descendant_process_running_for_target(observation_target, exe_name.to_string()).await {
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
        if !is_target_pid_alive(observation_target) {
            return;
        }
        if !descendant_process_running_for_target(observation_target, exe_name.to_string()).await {
            break;
        }
        tokio::time::sleep(GAMESCOPE_POLL_INTERVAL).await;
    }

    tracing::info!(
        exe = %exe_name,
        gamescope_pid = observation_target.pid,
        "gamescope watchdog: game exited, waiting for natural compositor shutdown"
    );

    tokio::time::sleep(GAMESCOPE_NATURAL_EXIT_GRACE).await;
    if !is_target_pid_alive(observation_target) {
        return;
    }

    let shutdown_target = observation_target;
    let descendants = collect_descendant_pids_task(shutdown_target).await;

    killed_flag.store(true, Ordering::Release);
    tracing::info!(
        gamescope_pid = shutdown_target.pid,
        "gamescope watchdog: sending SIGTERM"
    );
    signal_pid_task(shutdown_target.pid, None).await;

    tokio::time::sleep(GAMESCOPE_SIGTERM_WAIT).await;
    if !is_target_pid_alive(shutdown_target) {
        kill_remaining_descendants_task(descendants, shutdown_target.host_namespace).await;
        return;
    }

    tracing::warn!(
        gamescope_pid = shutdown_target.pid,
        "gamescope watchdog: sending SIGKILL"
    );
    signal_pid_task(shutdown_target.pid, Some("-KILL")).await;

    tokio::time::sleep(GAMESCOPE_DESCENDANT_CLEANUP_DELAY).await;
    kill_remaining_descendants_task(descendants, shutdown_target.host_namespace).await;
}

async fn descendant_process_running_for_target(target: ShutdownTarget, exe_name: String) -> bool {
    let exe_for_log = exe_name.clone();
    task::spawn_blocking(move || {
        if target.host_namespace {
            is_host_descendant_process_running(target.pid, &exe_name)
        } else {
            is_descendant_process_running(target.pid, &exe_name)
        }
    })
    .await
    .unwrap_or_else(|error| {
        tracing::warn!(
            %error,
            gamescope_pid = target.pid,
            exe = %exe_for_log,
            host_namespace = target.host_namespace,
            "gamescope watchdog: descendant scan task failed"
        );
        false
    })
}

async fn collect_descendant_pids_task(target: ShutdownTarget) -> Vec<u32> {
    task::spawn_blocking(move || {
        if target.host_namespace {
            collect_host_descendant_pids(target.pid)
        } else {
            collect_descendant_pids(target.pid)
        }
    })
    .await
    .unwrap_or_else(|error| {
        tracing::warn!(
            %error,
            gamescope_pid = target.pid,
            "gamescope watchdog: descendant collection task failed"
        );
        Vec::new()
    })
}

async fn kill_remaining_descendants_task(pids: Vec<u32>, host_namespace: bool) {
    if pids.is_empty() {
        return;
    }

    let descendant_count = pids.len();
    if let Err(error) =
        task::spawn_blocking(move || kill_remaining_descendants(&pids, host_namespace)).await
    {
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
        Ok(Ok(_status)) => {}
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

fn read_host_process_text(pid: u32, suffix: &str) -> Option<String> {
    let output = host_std_command("cat")
        .arg(format!("/proc/{pid}/{suffix}"))
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn host_process_matches_candidates(pid: u32, candidates: &[String]) -> bool {
    let Some(comm) = read_host_process_text(pid, "comm") else {
        return false;
    };
    let comm = comm.trim_end_matches('\n');
    if candidates.iter().any(|candidate| candidate == comm) {
        return true;
    }

    if comm.len() != TASK_COMM_LEN {
        return false;
    }

    let cmdline = read_host_process_text(pid, "cmdline");
    comm_matches_candidates(comm, cmdline.as_deref(), candidates)
}

fn is_host_descendant_process_running(root_pid: u32, exe_name: &str) -> bool {
    let candidates = process_name_candidates(exe_name);
    if candidates.is_empty() {
        return false;
    }

    collect_host_descendant_pids(root_pid)
        .into_iter()
        .any(|pid| host_process_matches_candidates(pid, &candidates))
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

fn kill_remaining_descendants(pids: &[u32], host_namespace: bool) {
    for &pid in pids {
        let alive = if host_namespace {
            is_host_pid_alive(pid)
        } else {
            is_pid_alive(pid)
        };
        if alive {
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
        collect_descendant_pids_from_children_map, comm_matches_candidates,
        host_process_line_parts, parse_status_ppid, process_name_candidates,
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

    #[test]
    fn host_process_line_parses_pid_and_comm_name() {
        assert_eq!(
            host_process_line_parts("4242\twitcher3.exe"),
            Some((4242, "witcher3.exe"))
        );
    }

    #[test]
    fn host_process_line_preserves_names_with_spaces() {
        assert_eq!(
            host_process_line_parts("4242\tWitcher 3.exe"),
            Some((4242, "Witcher 3.exe"))
        );
    }

    // Tests for resolve_watchdog_target_by_exe_name logic via pure helpers.
    //
    // resolve_watchdog_target_by_exe_name itself cannot be unit-tested end-to-end
    // because it calls collect_host_descendant_pids and host_process_matches_candidates,
    // which shell out to `ps` and `cat /proc/<pid>/...` on the host. The tests below
    // exercise the same matching logic (comm_matches_candidates + process_name_candidates)
    // and the descendant-walk (collect_descendant_pids_from_children_map) that the
    // function composes, giving equivalent coverage without spawning host processes.

    #[test]
    fn resolve_target_by_exe_name_matches_child_via_exact_comm() {
        // Simulate a children_map where pid 200 is a descendant of root 100,
        // and its comm matches the exe name exactly.
        let children_map = HashMap::from([(100u32, vec![200u32])]);
        let descendants = collect_descendant_pids_from_children_map(100, &children_map);
        let candidates = process_name_candidates("witcher3.exe");

        // Per-PID comm simulates host_process_matches_candidates + comm_matches_candidates.
        let comm_for_pid = |pid: u32| -> &str {
            match pid {
                200 => "witcher3.exe",
                _ => "other",
            }
        };
        let matched = descendants
            .iter()
            .copied()
            .find(|&pid| comm_matches_candidates(comm_for_pid(pid), None, &candidates));

        assert_eq!(matched, Some(200));
    }

    #[test]
    fn resolve_target_by_exe_name_no_match_returns_none() {
        // No descendant comm matches the exe name.
        let children_map = HashMap::from([(100u32, vec![201u32, 202u32])]);
        let descendants = collect_descendant_pids_from_children_map(100, &children_map);
        let candidates = process_name_candidates("game.exe");

        let comm_by_pid: HashMap<u32, &str> =
            HashMap::from([(201, "unrelated"), (202, "unrelated")]);
        let matched = descendants.iter().copied().find(|&pid| {
            let comm = comm_by_pid.get(&pid).copied().unwrap_or("unrelated");
            comm_matches_candidates(comm, None, &candidates)
        });

        assert_eq!(matched, None);
        // observed_descendants count and observed_gamescope_pid are still available for tracing.
        assert_eq!(descendants.len(), 2);
    }

    #[test]
    fn resolve_target_by_exe_name_cmdline_fallback_matches_truncated_comm() {
        // comm is truncated to 15 chars (TASK_COMM_LEN); cmdline carries the full
        // exe name with .exe suffix, which the fallback basename extraction resolves.
        let exe_name = "1234567890123456.exe"; // 20 chars — comm will be "123456789012345"
        let truncated_comm = "123456789012345"; // exactly TASK_COMM_LEN chars
        let cmdline = "/games/1234567890123456.exe\0--fullscreen";
        let candidates = process_name_candidates(exe_name);

        assert!(comm_matches_candidates(
            truncated_comm,
            Some(cmdline),
            &candidates
        ));
    }
}
