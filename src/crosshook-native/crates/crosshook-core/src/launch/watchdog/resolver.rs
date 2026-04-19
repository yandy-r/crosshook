use std::collections::HashMap;
use std::fs;
use std::path::Path;

use tokio::time;

use crate::platform::is_flatpak;

use super::descendants::collect_descendant_pids_from_children_map;
use super::{
    host::{
        collect_host_process_entries, host_process_entry_matches_candidates, read_host_text_file,
    },
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
        time::sleep(GAMESCOPE_HOST_PID_CAPTURE_WAIT_INTERVAL).await;
    }

    let (exe_target, descendants) = resolve_watchdog_target_by_host_ancestor_search(exe_name);

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

fn read_pid_capture_file(path: &Path) -> Option<u32> {
    let content = fs::read_to_string(path).ok().or_else(|| {
        if is_flatpak() {
            read_host_text_file(path)
        } else {
            None
        }
    })?;
    content.trim().parse::<u32>().ok().filter(|pid| *pid > 0)
}
