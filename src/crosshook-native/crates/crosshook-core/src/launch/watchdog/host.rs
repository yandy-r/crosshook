use std::collections::HashMap;

use crate::platform::host_std_command;

use super::host_table::collect_host_process_table;
pub(super) use super::host_table::HostProcessEntry;
use super::{
    descendants::collect_descendant_pids_from_children_map,
    process::{comm_matches_candidates, process_name_candidates},
    TASK_COMM_LEN,
};

#[derive(Debug, Default)]
pub(super) struct HostProcessProbe {
    pub(super) exact_matches: Vec<String>,
    pub(super) truncated_matches: Vec<String>,
}

impl HostProcessProbe {
    pub(super) fn has_match(&self) -> bool {
        !self.exact_matches.is_empty() || !self.truncated_matches.is_empty()
    }
}

pub(super) fn is_host_pid_alive(pid: u32) -> bool {
    host_std_command("test")
        .arg("-d")
        .arg(format!("/proc/{pid}"))
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub(super) fn collect_host_process_entries() -> Vec<HostProcessEntry> {
    collect_host_process_table().unwrap_or_default()
}

pub(super) fn host_process_entry_matches_candidates(
    entry: &HostProcessEntry,
    candidates: &[String],
) -> bool {
    if candidates.iter().any(|candidate| candidate == &entry.comm) {
        return true;
    }
    if entry.comm.len() != TASK_COMM_LEN {
        return false;
    }
    let cmdline = read_host_process_cmdline(entry.pid);
    comm_matches_candidates(&entry.comm, cmdline.as_deref(), candidates)
}

pub(super) fn collect_host_descendant_pids(root_pid: u32) -> Vec<u32> {
    let mut children_map: HashMap<u32, Vec<u32>> = HashMap::new();
    for entry in collect_host_process_entries() {
        children_map.entry(entry.ppid).or_default().push(entry.pid);
    }
    collect_descendant_pids_from_children_map(root_pid, &children_map)
}

pub(super) fn read_host_process_cmdline(pid: u32) -> Option<String> {
    let output = host_std_command("cat")
        .arg(format!("/proc/{pid}/cmdline"))
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Scans the host process table for `candidates`. `None` means neither host
/// `ps` nor the host `/proc` fallback listed any process, so nothing is known
/// about the host processes (distinct from "no match").
pub(super) fn probe_host_processes(candidates: &[String]) -> Option<HostProcessProbe> {
    let entries = collect_host_process_table()?;
    let mut probe = HostProcessProbe::default();
    for entry in &entries {
        let (pid, comm) = (entry.pid, entry.comm.as_str());
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
    Some(probe)
}

pub(super) fn read_host_process_text(pid: u32, suffix: &str) -> Option<String> {
    let output = host_std_command("cat")
        .arg(format!("/proc/{pid}/{suffix}"))
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub(super) fn host_process_matches_candidates(pid: u32, candidates: &[String]) -> bool {
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

pub(super) fn is_host_descendant_process_running(root_pid: u32, exe_name: &str) -> bool {
    let candidates = process_name_candidates(exe_name);
    if candidates.is_empty() {
        return false;
    }

    collect_host_descendant_pids(root_pid)
        .into_iter()
        .any(|pid| host_process_matches_candidates(pid, &candidates))
}
