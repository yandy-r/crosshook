use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::descendants::{collect_descendant_pids_from_children_map, parse_status_ppid};
use super::TASK_COMM_LEN;

pub(super) fn process_name_candidates(exe_name: &str) -> Vec<String> {
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

pub(super) fn process_matches_candidates(pid: u32, candidates: &[String]) -> bool {
    let pid_path = format!("/proc/{pid}");
    process_path_matches_candidates(Path::new(&pid_path), candidates)
}

pub(super) fn process_path_matches_candidates(pid_path: &Path, candidates: &[String]) -> bool {
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

pub(super) fn comm_matches_candidates(
    comm: &str,
    cmdline: Option<&str>,
    candidates: &[String],
) -> bool {
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

pub(super) fn is_pid_alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

pub(super) fn collect_descendant_pids(root_pid: u32) -> Vec<u32> {
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

pub(super) fn is_descendant_process_running(root_pid: u32, exe_name: &str) -> bool {
    let candidates = process_name_candidates(exe_name);
    if candidates.is_empty() {
        return false;
    }

    collect_descendant_pids(root_pid)
        .into_iter()
        .any(|pid| process_matches_candidates(pid, &candidates))
}
