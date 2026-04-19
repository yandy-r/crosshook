use std::collections::HashMap;
use std::path::Path;

use crate::platform::host_std_command;

use super::{
    descendants::collect_descendant_pids_from_children_map,
    process::{comm_matches_candidates, process_name_candidates},
    TASK_COMM_LEN,
};

#[derive(Debug, Default)]
pub(super) struct HostProcessProbe {
    pub(super) visible_count: usize,
    pub(super) exact_matches: Vec<String>,
    pub(super) truncated_matches: Vec<String>,
}

impl HostProcessProbe {
    pub(super) fn has_match(&self) -> bool {
        !self.exact_matches.is_empty() || !self.truncated_matches.is_empty()
    }
}

#[derive(Clone, Debug)]
pub(super) struct HostProcessEntry {
    pub(super) pid: u32,
    pub(super) ppid: u32,
    pub(super) comm: String,
}

pub(super) fn read_host_text_file(path: &Path) -> Option<String> {
    let output = host_std_command("cat").arg(path).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub(super) fn is_host_pid_alive(pid: u32) -> bool {
    host_std_command("test")
        .arg("-d")
        .arg(format!("/proc/{pid}"))
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub(super) fn parse_host_pid_and_ppid(line: &str) -> Option<(u32, u32)> {
    let mut fields = line.split('\t');
    let pid = fields.next()?.trim().parse().ok()?;
    let ppid = fields.next()?.trim().parse().ok()?;
    Some((pid, ppid))
}

pub(super) fn parse_host_process_entry(line: &str) -> Option<HostProcessEntry> {
    let mut fields = line.split('\t');
    let pid = fields.next()?.trim().parse().ok()?;
    let ppid = fields.next()?.trim().parse().ok()?;
    let comm = fields.next()?.trim();
    if comm.is_empty() {
        return None;
    }
    Some(HostProcessEntry {
        pid,
        ppid,
        comm: comm.to_string(),
    })
}

pub(super) fn collect_host_process_entries() -> Vec<HostProcessEntry> {
    let output = match host_std_command("ps")
        .args([
            "-ww",
            "-eo",
            "pid=",
            "-o",
            "ppid=",
            "-o",
            "comm=",
            "--delimiter",
            "\t",
        ])
        .output()
    {
        Ok(output) if output.status.success() => output,
        _ => return Vec::new(),
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter_map(parse_host_process_entry)
        .collect()
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

pub(super) fn host_process_line_parts(line: &str) -> Option<(u32, &str)> {
    let (pid, comm) = line.split_once('\t')?;
    let pid = pid.trim().parse().ok()?;
    let comm = comm.trim();
    if comm.is_empty() {
        return None;
    }
    Some((pid, comm))
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

pub(super) fn probe_host_processes(candidates: &[String]) -> HostProcessProbe {
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
