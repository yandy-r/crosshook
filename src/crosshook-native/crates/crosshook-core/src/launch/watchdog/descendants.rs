use std::collections::{HashMap, VecDeque};
use std::fs;

pub(super) fn parse_status_ppid(status: &str) -> Option<u32> {
    status
        .lines()
        .find_map(|line| line.strip_prefix("PPid:\t"))
        .and_then(|ppid| ppid.trim().parse::<u32>().ok())
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

pub(super) fn collect_descendant_pids_from_children_map(
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
