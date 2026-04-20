use std::env;
use std::fs;
use std::process::Command;

use crate::platform;

/// Collects system information from `/proc` and environment variables.
#[allow(clippy::vec_init_then_push)]
pub(super) fn collect_system_info() -> String {
    let mut lines = Vec::new();

    lines.push("=== Kernel ===".to_string());
    lines.push(read_file_lossy("/proc/version").unwrap_or_else(|| "(unavailable)".to_string()));
    lines.push(String::new());

    lines.push("=== OS Release ===".to_string());
    lines.push(read_file_lossy("/etc/os-release").unwrap_or_else(|| "(unavailable)".to_string()));
    lines.push(String::new());

    lines.push("=== CPU ===".to_string());
    if let Some(cpuinfo) = read_file_lossy("/proc/cpuinfo") {
        for line in cpuinfo.lines() {
            if line.starts_with("model name") {
                lines.push(line.to_string());
                break;
            }
        }
    } else {
        lines.push("(unavailable)".to_string());
    }
    lines.push(String::new());

    lines.push("=== Memory ===".to_string());
    if let Some(meminfo) = read_file_lossy("/proc/meminfo") {
        for line in meminfo.lines() {
            if line.starts_with("MemTotal") {
                lines.push(line.to_string());
                break;
            }
        }
    } else {
        lines.push("(unavailable)".to_string());
    }
    lines.push(String::new());

    lines.push("=== GPU ===".to_string());
    let lspci_output = if platform::is_flatpak() {
        platform::host_std_command("lspci").output()
    } else {
        Command::new("lspci").output()
    };
    match lspci_output {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                if stderr.is_empty() {
                    lines.push(format!("(lspci failed: {})", output.status));
                } else {
                    lines.push(format!("(lspci failed: {}: {stderr})", output.status));
                }
            } else {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let gpu_lines: Vec<&str> = stdout
                    .lines()
                    .filter(|line| {
                        let lower = line.to_lowercase();
                        lower.contains("vga") || lower.contains("3d controller")
                    })
                    .collect();
                if gpu_lines.is_empty() {
                    lines.push("(no VGA/3D devices found)".to_string());
                } else {
                    for gpu_line in gpu_lines {
                        lines.push(gpu_line.to_string());
                    }
                }
            }
        }
        Err(error) => lines.push(format!("(lspci not available: {error})")),
    }

    if let Some(nvidia) = read_file_lossy("/proc/driver/nvidia/version") {
        lines.push(String::new());
        lines.push("=== NVIDIA Driver ===".to_string());
        lines.push(nvidia);
    }
    lines.push(String::new());

    lines.push("=== Desktop Environment ===".to_string());
    lines.push(format!(
        "XDG_CURRENT_DESKTOP: {}",
        env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "(unset)".to_string())
    ));
    lines.push(format!(
        "XDG_SESSION_TYPE: {}",
        env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "(unset)".to_string())
    ));
    lines.push(format!(
        "WAYLAND_DISPLAY: {}",
        env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "(unset)".to_string())
    ));

    lines.join("\n")
}

fn read_file_lossy(path: &str) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_system_info_returns_nonempty_string() {
        let info = collect_system_info();
        assert!(!info.is_empty());
        assert!(info.contains("Kernel") || info.contains("kernel"));
    }
}
