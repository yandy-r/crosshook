use std::fs;
use std::time::Duration;

use crate::platform::is_flatpak;

mod descendants;
mod host;
mod process;
mod resolver;
mod tasks;

pub use tasks::gamescope_watchdog;

pub(super) const GAMESCOPE_STARTUP_POLL_ITERATIONS: u32 = 60;
pub(super) const GAMESCOPE_POLL_INTERVAL: Duration = Duration::from_secs(2);
pub(super) const GAMESCOPE_NATURAL_EXIT_GRACE: Duration = Duration::from_secs(5);
pub(super) const GAMESCOPE_SIGTERM_WAIT: Duration = Duration::from_secs(3);
pub(super) const GAMESCOPE_DESCENDANT_CLEANUP_DELAY: Duration = Duration::from_millis(500);
pub(super) const GAMESCOPE_HOST_PID_CAPTURE_WAIT_ITERATIONS: u32 = 20;
pub(super) const GAMESCOPE_HOST_PID_CAPTURE_WAIT_INTERVAL: Duration = Duration::from_millis(200);
pub(super) const TASK_COMM_LEN: usize = 15;

#[derive(Clone, Copy, Debug)]
pub(super) struct ShutdownTarget {
    pub(super) pid: u32,
    pub(super) host_namespace: bool,
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
    let candidates = process::process_name_candidates(exe_name);
    if candidates.is_empty() {
        return false;
    }

    let mut sandbox_match = false;
    if let Ok(proc_dir) = fs::read_dir("/proc") {
        for entry in proc_dir.flatten() {
            let dir_name = entry.file_name();
            let dir_name_str = dir_name.to_string_lossy();

            if dir_name_str.is_empty() || !dir_name_str.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }

            if process::process_path_matches_candidates(&entry.path(), &candidates) {
                sandbox_match = true;
                break;
            }
        }
    }

    let host_probe = if is_flatpak() {
        Some(host::probe_host_processes(&candidates))
    } else {
        None
    };

    sandbox_match
        || host_probe
            .as_ref()
            .is_some_and(host::HostProcessProbe::has_match)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{
        descendants::{collect_descendant_pids_from_children_map, parse_status_ppid},
        host::{host_process_line_parts, parse_host_process_entry},
        process::{comm_matches_candidates, process_name_candidates},
        resolver::resolve_unique_gamescope_ancestor,
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

    #[test]
    fn host_process_entry_parses_pid_ppid_and_comm() {
        let entry = parse_host_process_entry("4242\t41\tgamescope");

        assert_eq!(entry.as_ref().map(|entry| entry.pid), Some(4242));
        assert_eq!(entry.as_ref().map(|entry| entry.ppid), Some(41));
        assert_eq!(
            entry.as_ref().map(|entry| entry.comm.as_str()),
            Some("gamescope")
        );
    }

    // Tests for the host-process matching primitives used by the watchdog fallback.
    //
    // The final resolver shells out to host `ps` and `/proc/<pid>/...`, so these
    // tests stay at the pure-helper layer: name matching, descendant walks, and
    // gamescope-ancestor selection.

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

    #[test]
    fn unique_gamescope_ancestor_prefers_real_host_tree_over_wrapper_pid() {
        let parent_by_pid = HashMap::from([
            (200u32, 1u32),   // real host gamescope
            (300u32, 200u32), // wine/preloader descendant
            (301u32, 300u32), // game.exe descendant
            (999u32, 1u32),   // unrelated sandbox-visible wrapper pid
        ]);

        assert_eq!(
            resolve_unique_gamescope_ancestor(&parent_by_pid, &[301], &[200]),
            Some(200)
        );
    }

    #[test]
    fn unique_gamescope_ancestor_rejects_ambiguous_gamescope_matches() {
        let parent_by_pid = HashMap::from([
            (200u32, 1u32),
            (201u32, 1u32),
            (300u32, 200u32),
            (301u32, 201u32),
        ]);

        assert_eq!(
            resolve_unique_gamescope_ancestor(&parent_by_pid, &[300, 301], &[200, 201]),
            None
        );
    }

    #[test]
    fn unique_gamescope_ancestor_ignores_same_named_process_without_gamescope_parent() {
        let parent_by_pid = HashMap::from([
            (200u32, 1u32),
            (300u32, 200u32),
            (301u32, 300u32),
            (900u32, 1u32),
        ]);

        assert_eq!(
            resolve_unique_gamescope_ancestor(&parent_by_pid, &[301, 900], &[200]),
            Some(200)
        );
    }
}
