//! Host tool detail probing for onboarding/dashboard surfaces.
//!
//! This module mirrors the readiness catalog lookup flow and host command
//! execution semantics already used elsewhere in CrossHook. It intentionally
//! degrades to `None` fields instead of raising errors so UI callers can
//! request detail probes opportunistically.

use std::io::Read;
use std::process::Stdio;
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::launch::runtime_helpers::resolve_umu_run_path;
use crate::platform;

use super::global_readiness_catalog;

const DETAIL_PROBE_TIMEOUT: Duration = Duration::from_millis(1500);
const DETAIL_PROBE_POLL_INTERVAL: Duration = Duration::from_millis(25);

const GENERIC_VERSION_ARG_CANDIDATES: &[&[&str]] = &[&["--version"], &["-V"], &["version"]];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostToolDetails {
    pub tool_id: String,
    #[serde(default)]
    pub tool_version: Option<String>,
    #[serde(default)]
    pub resolved_path: Option<String>,
}

impl HostToolDetails {
    fn unavailable(tool_id: &str) -> Self {
        Self {
            tool_id: tool_id.to_string(),
            tool_version: None,
            resolved_path: None,
        }
    }
}

/// Probe a catalog-defined host tool for its resolved runtime path and version.
///
/// Failures are soft by design: missing catalog rows, missing binaries, probe
/// timeouts, and unparsable output all return `None` detail fields.
pub fn probe_host_tool_details(tool_id: &str) -> HostToolDetails {
    let Some(entry) = global_readiness_catalog().find_by_id(tool_id) else {
        return HostToolDetails::unavailable(tool_id);
    };

    let mut details = HostToolDetails::unavailable(&entry.tool_id);
    let Some(resolved_path) = resolve_tool_binary_path(&entry.tool_id, &entry.binary_name) else {
        return details;
    };

    details.resolved_path = Some(resolved_path.clone());
    details.tool_version = probe_tool_version(&entry.tool_id, &resolved_path);
    details
}

fn resolve_tool_binary_path(tool_id: &str, binary_name: &str) -> Option<String> {
    let trimmed_binary = binary_name.trim();
    if trimmed_binary.is_empty() {
        return None;
    }

    if tool_id == "umu_run" {
        return resolve_umu_run_path().and_then(normalize_and_validate_resolved_path);
    }

    if trimmed_binary.contains('/') {
        return normalize_and_validate_resolved_path(trimmed_binary.to_string());
    }

    if !platform::is_safe_host_path_lookup_name(trimmed_binary)
        || !platform::host_command_exists(trimmed_binary)
    {
        return None;
    }

    let mut command = platform::host_std_command("which");
    command
        .arg(trimmed_binary)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }

    let resolved = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())?
        .to_string();

    normalize_and_validate_resolved_path(resolved)
}

fn normalize_and_validate_resolved_path(path: String) -> Option<String> {
    let normalized = platform::normalize_flatpak_host_path(&path);
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return None;
    }

    platform::normalized_path_is_executable_file_on_host(trimmed).then(|| trimmed.to_string())
}

fn probe_tool_version(tool_id: &str, resolved_path: &str) -> Option<String> {
    for args in version_arg_candidates(tool_id) {
        let output = run_version_probe(resolved_path, args)?;
        if let Some(version) = parse_version_output(tool_id, &output) {
            return Some(version);
        }
    }

    None
}

fn version_arg_candidates(tool_id: &str) -> Vec<&'static [&'static str]> {
    let specific: &[&[&str]] = match tool_id {
        "gamescope" => &[&["--version"]],
        "mangohud" => &[&["--version"]],
        "gamemode" => &[&["--version"], &["-v"]],
        "umu_run" => &[&["--version"], &["version"]],
        "winetricks" => &[&["--version"], &["-V"]],
        "protontricks" => &[&["--version"], &["-V"]],
        _ => &[],
    };

    let mut candidates = Vec::with_capacity(specific.len() + GENERIC_VERSION_ARG_CANDIDATES.len());
    for candidate in specific {
        candidates.push(*candidate);
    }
    for candidate in GENERIC_VERSION_ARG_CANDIDATES {
        if !candidates.contains(candidate) {
            candidates.push(*candidate);
        }
    }

    candidates
}

fn run_version_probe(program: &str, args: &[&str]) -> Option<String> {
    let mut command = platform::host_std_command(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command.spawn().ok()?;
    let deadline = Instant::now() + DETAIL_PROBE_TIMEOUT;

    loop {
        match child.try_wait() {
            Ok(Some(_status)) => break,
            Ok(None) if Instant::now() < deadline => {
                thread::sleep(DETAIL_PROBE_POLL_INTERVAL);
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }

    let stdout = read_child_pipe(child.stdout.take());
    let stderr = read_child_pipe(child.stderr.take());

    let mut combined = String::new();
    if !stdout.trim().is_empty() {
        combined.push_str(stdout.trim());
    }
    if !stderr.trim().is_empty() {
        if !combined.is_empty() {
            combined.push('\n');
        }
        combined.push_str(stderr.trim());
    }

    (!combined.trim().is_empty()).then_some(combined)
}

fn read_child_pipe(pipe: Option<impl Read>) -> String {
    let Some(mut pipe) = pipe else {
        return String::new();
    };

    let mut buffer = Vec::new();
    if pipe.read_to_end(&mut buffer).is_err() {
        return String::new();
    }
    String::from_utf8_lossy(&buffer).into_owned()
}

fn parse_version_output(tool_id: &str, output: &str) -> Option<String> {
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .find_map(|line| parse_version_line(tool_id, line))
}

fn parse_version_line(tool_id: &str, line: &str) -> Option<String> {
    let prefixed_line = match tool_id {
        "gamescope" => line.strip_prefix("gamescope version ").unwrap_or(line),
        "mangohud" => line
            .strip_prefix("MangoHud ")
            .or_else(|| line.strip_prefix("mangohud "))
            .unwrap_or(line),
        "gamemode" => line
            .strip_prefix("gamemoderun ")
            .or_else(|| line.strip_prefix("gamemode "))
            .or_else(|| line.strip_prefix("GameMode "))
            .unwrap_or(line),
        "umu_run" => line
            .strip_prefix("umu-run ")
            .or_else(|| line.strip_prefix("umu_run "))
            .unwrap_or(line),
        "winetricks" => line.strip_prefix("winetricks ").unwrap_or(line),
        "protontricks" => line.strip_prefix("protontricks ").unwrap_or(line),
        _ => line,
    };

    extract_version_token(prefixed_line).or_else(|| extract_version_token(line))
}

fn extract_version_token(line: &str) -> Option<String> {
    for raw_token in line.split_whitespace() {
        let token = raw_token
            .trim_matches(|ch: char| matches!(ch, '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';'))
            .trim_start_matches("version=")
            .trim_start_matches("version:")
            .trim();

        if token.is_empty() || token.eq_ignore_ascii_case("version") {
            continue;
        }

        let normalized = strip_version_prefix(token);
        if normalized.chars().any(|ch| ch.is_ascii_digit())
            && normalized.chars().all(is_allowed_version_char)
        {
            return Some(normalized.to_string());
        }
    }

    None
}

fn strip_version_prefix(token: &str) -> &str {
    token
        .strip_prefix(['v', 'V'])
        .filter(|remainder| {
            remainder
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_digit())
        })
        .unwrap_or(token)
}

fn is_allowed_version_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | '+')
}

#[cfg(test)]
mod tests {
    use super::parse_version_line;

    #[derive(Debug)]
    struct VersionParseCase {
        tool_id: &'static str,
        line: &'static str,
        expected: &'static str,
    }

    #[test]
    fn parse_version_line_supports_known_host_tools() {
        let cases = [
            VersionParseCase {
                tool_id: "gamescope",
                line: "gamescope version 3.15.13 (gcc 13.2.1)",
                expected: "3.15.13",
            },
            VersionParseCase {
                tool_id: "mangohud",
                line: "MangoHud v0.8.1",
                expected: "0.8.1",
            },
            VersionParseCase {
                tool_id: "gamemode",
                line: "gamemoderun 1.8.2",
                expected: "1.8.2",
            },
            VersionParseCase {
                tool_id: "umu_run",
                line: "umu-run 1.2.5",
                expected: "1.2.5",
            },
            VersionParseCase {
                tool_id: "winetricks",
                line: "winetricks 20250102-next - sha256sum: deadbeef",
                expected: "20250102-next",
            },
            VersionParseCase {
                tool_id: "protontricks",
                line: "protontricks (1.12.0)",
                expected: "1.12.0",
            },
        ];

        for case in cases {
            let parsed = parse_version_line(case.tool_id, case.line);
            assert_eq!(
                parsed.as_deref(),
                Some(case.expected),
                "failed to parse {case:?}"
            );
        }
    }
}
