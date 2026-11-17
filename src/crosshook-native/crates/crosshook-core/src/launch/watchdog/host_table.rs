//! Host process table for the Flatpak watchdog paths.
//!
//! The table comes from host `ps -eo pid= -o ppid= -o uid= -o comm=`, which works on
//! every procps-ng release (`--delimiter` exists only from 4.0.6, so it is
//! not used). When `ps` cannot be run or yields nothing usable, the table is
//! read from the host's `/proc/<pid>/stat` files instead, with each process's
//! owner taken from `stat -c '%u %n' /proc/<pid>` (missing owners stay `None`).

use std::io;
use std::process::Output;

use crate::platform::host_std_command;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct HostProcessEntry {
    pub(super) pid: u32,
    pub(super) ppid: u32,
    /// Effective user id from `ps`; `None` when read from `/proc/<pid>/stat`,
    /// which does not carry it.
    pub(super) uid: Option<u32>,
    pub(super) comm: String,
}

const PS_ARGS: [&str; 9] = [
    "-ww", "-eo", "pid=", "-o", "ppid=", "-o", "uid=", "-o", "comm=",
];
/// Prints one `U <uid> /proc/<pid>` owner line per process, then every
/// `/proc/<pid>/stat` line. Both commands keep going past a process that
/// exited mid-glob; their exit statuses are ignored and only the parsed output
/// counts.
const PROC_STAT_SCRIPT: &str =
    "stat -c 'U %u %n' /proc/[0-9]* 2>/dev/null; cat /proc/[0-9]*/stat 2>/dev/null";

/// Reads the host process table. `None` means neither `ps` nor the `/proc`
/// fallback produced any process, so nothing is known about the host.
pub(super) fn collect_host_process_table() -> Option<Vec<HostProcessEntry>> {
    collect_host_process_table_with(
        || host_std_command("ps").args(PS_ARGS).output(),
        || {
            host_std_command("sh")
                .args(["-c", PROC_STAT_SCRIPT])
                .output()
        },
    )
}

pub(super) fn collect_host_process_table_with(
    run_ps: impl FnOnce() -> io::Result<Output>,
    read_proc_stats: impl FnOnce() -> io::Result<Output>,
) -> Option<Vec<HostProcessEntry>> {
    match run_ps() {
        Ok(output) if output.status.success() => {
            let entries = parse_lines(&output.stdout, parse_ps_line);
            if !entries.is_empty() {
                return Some(entries);
            }
            tracing::warn!("host ps returned no parseable processes; reading host /proc");
        }
        Ok(output) => tracing::warn!(
            status = %output.status,
            stderr = %String::from_utf8_lossy(&output.stderr).trim(),
            "host ps failed; reading host /proc"
        ),
        Err(error) => tracing::warn!(%error, "host ps could not run; reading host /proc"),
    }

    match read_proc_stats() {
        Ok(output) => {
            let entries = parse_proc_fallback(&output.stdout);
            if entries.is_empty() {
                tracing::warn!(
                    status = %output.status,
                    "host /proc fallback returned no processes"
                );
                return None;
            }
            Some(entries)
        }
        Err(error) => {
            tracing::warn!(%error, "host /proc fallback could not run");
            None
        }
    }
}

fn parse_lines(
    stdout: &[u8],
    parse: fn(&str) -> Option<HostProcessEntry>,
) -> Vec<HostProcessEntry> {
    String::from_utf8_lossy(stdout)
        .lines()
        .filter_map(parse)
        .collect()
}

/// Parses one `ps -eo pid= -o ppid= -o uid= -o comm=` line. procps-ng before
/// 4.0.6 pads the columns with spaces (`  4242    41  1000 Witcher 3.exe`); a
/// tab delimited line (`4242\t41\t1000\tgamescope`) parses the same way. The
/// command name is everything after the third field, so it may contain spaces.
pub(super) fn parse_ps_line(line: &str) -> Option<HostProcessEntry> {
    let (pid, rest) = split_field(line)?;
    let (ppid, rest) = split_field(rest)?;
    let (uid, comm) = split_field(rest)?;
    let mut parsed = entry(pid, ppid, comm)?;
    parsed.uid = Some(uid.trim().parse().ok()?);
    Some(parsed)
}

fn split_field(text: &str) -> Option<(&str, &str)> {
    let text = text.trim_start();
    let end = text.find(char::is_whitespace)?;
    Some((&text[..end], &text[end..]))
}

/// Parses the `/proc` fallback output: `U <uid> /proc/<pid>` owner lines and
/// `/proc/<pid>/stat` lines, joined by pid.
fn parse_proc_fallback(stdout: &[u8]) -> Vec<HostProcessEntry> {
    let text = String::from_utf8_lossy(stdout);
    let owners: std::collections::HashMap<u32, u32> =
        text.lines().filter_map(parse_owner_line).collect();
    text.lines()
        .filter(|line| !line.starts_with("U "))
        .filter_map(parse_proc_stat_line)
        .map(|mut entry| {
            entry.uid = owners.get(&entry.pid).copied();
            entry
        })
        .collect()
}

/// Parses `U <uid> /proc/<pid>` into `(pid, uid)`.
fn parse_owner_line(line: &str) -> Option<(u32, u32)> {
    let mut fields = line.strip_prefix("U ")?.split_whitespace();
    let uid = fields.next()?.parse().ok()?;
    let pid = fields.next()?.strip_prefix("/proc/")?.parse().ok()?;
    Some((pid, uid))
}

/// Parses one `/proc/<pid>/stat` line: `pid (comm) state ppid ...`. The
/// command name sits between the first `(` and the last `)` because it may
/// itself contain spaces and parentheses.
pub(super) fn parse_proc_stat_line(line: &str) -> Option<HostProcessEntry> {
    let open = line.find('(')?;
    let close = line.rfind(')')?;
    if close < open {
        return None;
    }
    let mut fields = line[close + 1..].split_whitespace();
    let _state = fields.next()?;
    let ppid = fields.next()?;
    entry(&line[..open], ppid, &line[open + 1..close])
}

fn entry(pid: &str, ppid: &str, comm: &str) -> Option<HostProcessEntry> {
    let comm = comm.trim();
    if comm.is_empty() {
        return None;
    }
    Some(HostProcessEntry {
        pid: pid.trim().parse().ok()?,
        ppid: ppid.trim().parse().ok()?,
        uid: None,
        comm: comm.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;

    use super::*;

    fn entry(pid: u32, ppid: u32, comm: &str) -> HostProcessEntry {
        HostProcessEntry {
            pid,
            ppid,
            uid: None,
            comm: comm.to_string(),
        }
    }

    fn with_uid(entries: Vec<HostProcessEntry>) -> Vec<HostProcessEntry> {
        entries
            .into_iter()
            .map(|entry| HostProcessEntry {
                uid: Some(1000),
                ..entry
            })
            .collect()
    }

    fn output(exit_code: i32, stdout: &str, stderr: &str) -> io::Result<Output> {
        Ok(Output {
            status: ExitStatus::from_raw(exit_code << 8),
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        })
    }

    const OLD_PS: &str = "      1       0  1000 systemd\n   4242      41  1000 Witcher 3.exe\n  31337    4242  1000 gamescope\n";
    const NEW_PS: &str =
        "1\t0\t1000\tsystemd\n4242\t41\t1000\tWitcher 3.exe\n31337\t4242\t1000\tgamescope\n";
    const PROC_STATS: &str = "1 (systemd) S 0 1 1 0 -1\n4242 (Witcher 3.exe) S 41 4242 4242 0\n31337 (gamescope) R 4242 31337 0 0\n";

    fn expected() -> Vec<HostProcessEntry> {
        vec![
            entry(1, 0, "systemd"),
            entry(4242, 41, "Witcher 3.exe"),
            entry(31337, 4242, "gamescope"),
        ]
    }

    #[test]
    fn parses_space_padded_ps_output_from_procps_before_4_0_6() {
        assert_eq!(
            parse_lines(OLD_PS.as_bytes(), parse_ps_line),
            with_uid(expected())
        );
    }

    #[test]
    fn parses_tab_delimited_ps_output_from_procps_4_0_6() {
        assert_eq!(
            parse_lines(NEW_PS.as_bytes(), parse_ps_line),
            with_uid(expected())
        );
    }

    #[test]
    fn ps_line_without_comm_or_with_bad_numbers_is_skipped() {
        assert_eq!(parse_ps_line("  4242    41  1000"), None);
        assert_eq!(parse_ps_line("  4242    41  1000   "), None);
        assert_eq!(parse_ps_line("  abc    41  1000 game.exe"), None);
        assert_eq!(parse_ps_line("  4242    41  root game.exe"), None);
        assert_eq!(parse_ps_line(""), None);
    }

    #[test]
    fn parses_proc_stat_lines_with_spaces_and_parens_in_comm() {
        assert_eq!(
            parse_lines(PROC_STATS.as_bytes(), parse_proc_stat_line),
            expected()
        );
        assert_eq!(
            parse_proc_stat_line("77 (a) b (c)) S 5 77 77 0"),
            Some(entry(77, 5, "a) b (c)"))
        );
        assert_eq!(parse_proc_stat_line("77 (game.exe)"), None);
        assert_eq!(parse_proc_stat_line("77 )x( S 5"), None);
    }

    #[test]
    fn proc_fallback_takes_owners_from_stat_lines() {
        let stdout = format!("U 1000 /proc/31337\nU 0 /proc/1\n{PROC_STATS}");
        let entries = parse_proc_fallback(stdout.as_bytes());
        let uid_of = |pid| entries.iter().find(|entry| entry.pid == pid).unwrap().uid;
        assert_eq!(uid_of(31337), Some(1000));
        assert_eq!(uid_of(1), Some(0));
        assert_eq!(uid_of(4242), None, "a missing owner line stays unknown");
    }

    #[test]
    fn successful_ps_does_not_read_proc() {
        let fallback_ran = Cell::new(false);
        let table = collect_host_process_table_with(
            || output(0, OLD_PS, ""),
            || {
                fallback_ran.set(true);
                output(0, PROC_STATS, "")
            },
        );
        assert_eq!(table, Some(with_uid(expected())));
        assert!(!fallback_ran.get());
    }

    #[test]
    fn ps_rejecting_delimiter_style_options_falls_back_to_proc() {
        let table = collect_host_process_table_with(
            || output(1, "", "error: unknown gnu long option\n"),
            || output(1, PROC_STATS, ""),
        );
        assert_eq!(table, Some(expected()));
    }

    #[test]
    fn ps_spawn_error_or_unparseable_output_falls_back_to_proc() {
        let spawn_error = collect_host_process_table_with(
            || Err(io::Error::new(io::ErrorKind::NotFound, "ps")),
            || output(0, PROC_STATS, ""),
        );
        assert_eq!(spawn_error, Some(expected()));

        let garbage = collect_host_process_table_with(
            || output(0, "PID PPID COMMAND\n", ""),
            || output(0, PROC_STATS, ""),
        );
        assert_eq!(garbage, Some(expected()));
    }

    #[test]
    fn table_is_unknown_when_ps_and_proc_both_fail() {
        assert_eq!(
            collect_host_process_table_with(
                || output(1, "", "error: unknown gnu long option\n"),
                || output(1, "", ""),
            ),
            None
        );
        assert_eq!(
            collect_host_process_table_with(
                || output(1, "", ""),
                || Err(io::Error::new(io::ErrorKind::NotFound, "sh")),
            ),
            None
        );
    }
}
