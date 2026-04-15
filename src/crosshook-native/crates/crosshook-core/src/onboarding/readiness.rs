use std::fs;
use std::path::PathBuf;

use crate::launch::runtime_helpers::resolve_umu_run_path;
use crate::onboarding::{ReadinessCheckResult, UmuInstallGuidance};
use crate::profile::health::{HealthIssue, HealthIssueSeverity};
use crate::steam::{discover_compat_tools, discover_steam_root_candidates, ProtonInstall};

const UMU_LAUNCHER_DOCS_URL: &str = "https://github.com/Open-Wine-Components/umu-launcher";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostDistroFamily {
    Arch,
    Nobara,
    Fedora,
    Debian,
    Nix,
    Unknown,
}

#[derive(Debug, Clone)]
struct UmuInstallAdvice {
    guidance: UmuInstallGuidance,
    remediation: String,
}

fn read_host_os_release() -> Option<String> {
    read_host_os_release_with(
        crate::platform::is_flatpak(),
        |path| fs::read_to_string(path).ok(),
        || {
            crate::platform::host_std_command("cat")
                .arg("/etc/os-release")
                .output()
                .ok()
                .and_then(|output| output.status.success().then_some(output.stdout))
                .and_then(|stdout| String::from_utf8(stdout).ok())
        },
    )
}

fn read_host_os_release_with<F, G>(
    is_flatpak: bool,
    mut read_file: F,
    read_via_host_command: G,
) -> Option<String>
where
    F: FnMut(&str) -> Option<String>,
    G: FnOnce() -> Option<String>,
{
    if let Some(content) = read_file("/run/host/etc/os-release") {
        return Some(content);
    }
    if is_flatpak {
        return read_via_host_command();
    }
    read_file("/etc/os-release")
}

fn detect_host_distro_family() -> HostDistroFamily {
    detect_host_distro_family_from_os_release(read_host_os_release().as_deref())
}

fn detect_host_distro_family_from_os_release(os_release: Option<&str>) -> HostDistroFamily {
    let Some(os_release) = os_release else {
        return HostDistroFamily::Unknown;
    };

    let mut distro_tokens = Vec::new();
    for line in os_release.lines() {
        let Some((key, raw_value)) = line.split_once('=') else {
            continue;
        };
        if key != "ID" && key != "ID_LIKE" {
            continue;
        }
        let normalized = raw_value
            .trim()
            .trim_matches(|ch| ch == '"' || ch == '\'')
            .to_ascii_lowercase();
        distro_tokens.extend(normalized.split_whitespace().map(str::to_string));
    }

    if distro_tokens
        .iter()
        .any(|token| matches!(token.as_str(), "arch" | "manjaro" | "endeavouros"))
    {
        return HostDistroFamily::Arch;
    }
    if distro_tokens.iter().any(|token| token == "nobara") {
        return HostDistroFamily::Nobara;
    }
    if distro_tokens
        .iter()
        .any(|token| matches!(token.as_str(), "fedora" | "rhel" | "centos"))
    {
        return HostDistroFamily::Fedora;
    }
    if distro_tokens
        .iter()
        .any(|token| matches!(token.as_str(), "debian" | "ubuntu" | "linuxmint" | "pop"))
    {
        return HostDistroFamily::Debian;
    }
    if distro_tokens
        .iter()
        .any(|token| matches!(token.as_str(), "nixos" | "nix"))
    {
        return HostDistroFamily::Nix;
    }

    HostDistroFamily::Unknown
}

fn build_umu_install_advice(distro_family: HostDistroFamily) -> UmuInstallAdvice {
    let (description, install_command, alternatives) = match distro_family {
        HostDistroFamily::Arch => (
            "Install umu-launcher on your Arch-based host to enable improved Proton runtime bootstrapping for non-Steam launches.",
            "sudo pacman -S umu-launcher",
            "If the package is unavailable in your mirror set, use the upstream docs for source or user-level install options.",
        ),
        HostDistroFamily::Nobara => (
            "Install umu-launcher on your Nobara host to enable improved Proton runtime bootstrapping for non-Steam launches.",
            "sudo dnf install umu-launcher",
            "If the packaged build is unavailable, fall back to the upstream docs for user-level or source installs.",
        ),
        HostDistroFamily::Fedora => (
            "Install umu-launcher on your Fedora host to enable improved Proton runtime bootstrapping for non-Steam launches.",
            "sudo dnf install pipx && pipx install umu-launcher",
            "If you prefer a packaged build, check the upstream docs for current Fedora/Nobara packaging guidance.",
        ),
        HostDistroFamily::Debian => (
            "Install umu-launcher on your Debian/Ubuntu host to enable improved Proton runtime bootstrapping for non-Steam launches.",
            "sudo apt install pipx && pipx install umu-launcher",
            "If pipx is not suitable on this system, use the upstream docs for source or user-level install options.",
        ),
        HostDistroFamily::Nix => (
            "Install umu-launcher on your Nix host to enable improved Proton runtime bootstrapping for non-Steam launches.",
            "nix profile install nixpkgs#umu-launcher",
            "Alternative: add `pkgs.umu-launcher` to your NixOS or Home Manager packages.",
        ),
        HostDistroFamily::Unknown => (
            "Install umu-launcher on your host to enable improved Proton runtime bootstrapping for non-Steam launches.",
            "pipx install umu-launcher",
            "Other options include upstream source, user-level, or uv-based installs described in the project docs.",
        ),
    };

    let guidance = UmuInstallGuidance {
        install_command: install_command.to_string(),
        docs_url: UMU_LAUNCHER_DOCS_URL.to_string(),
        description: description.to_string(),
    };
    let remediation = format!(
        "Install umu-launcher on your host: `{}`. {} See {} for full instructions.",
        guidance.install_command, alternatives, guidance.docs_url,
    );

    UmuInstallAdvice {
        guidance,
        remediation,
    }
}

/// Replace the home directory prefix with `~` for cleaner display paths.
fn home_to_tilde(path_str: &str) -> String {
    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() && path_str.starts_with(&home) {
            return format!("~{}", &path_str[home.len()..]);
        }
    }
    path_str.to_string()
}

/// Run all system-level first-run readiness checks synchronously.
///
/// Checks: Steam installed, Proton available, game launched once, trainer available,
/// umu-run available.
/// Path strings in the returned `HealthIssue` values have the home directory
/// replaced with `~` for display safety.
pub fn check_system_readiness() -> ReadinessCheckResult {
    let mut diagnostics: Vec<String> = Vec::new();
    let steam_roots = discover_steam_root_candidates("", &mut diagnostics);
    tracing::debug!(
        root_count = steam_roots.len(),
        "Steam root discovery complete"
    );
    let mut proton_diagnostics: Vec<String> = Vec::new();
    let proton_tools = discover_compat_tools(&steam_roots, &mut proton_diagnostics);
    tracing::debug!(count = proton_tools.len(), "Proton tools discovered");
    evaluate_checks(&steam_roots, &proton_tools)
}

/// Core evaluation logic — separated so tests can supply explicit Proton tool lists
/// to avoid depending on system-level compat tool directories.
fn evaluate_checks(
    steam_roots: &[PathBuf],
    proton_tools: &[ProtonInstall],
) -> ReadinessCheckResult {
    let umu_path = resolve_umu_run_path();
    let is_flatpak = crate::platform::is_flatpak();
    evaluate_checks_inner(steam_roots, proton_tools, umu_path, is_flatpak)
}

/// Inner evaluation logic with injectable umu resolution and platform context so
/// unit tests can exercise both the native and Flatpak code paths deterministically.
fn evaluate_checks_inner(
    steam_roots: &[PathBuf],
    proton_tools: &[ProtonInstall],
    umu_path: Option<String>,
    is_flatpak: bool,
) -> ReadinessCheckResult {
    let mut checks: Vec<HealthIssue> = Vec::new();

    // Check 1: Steam installed
    if steam_roots.is_empty() {
        checks.push(HealthIssue {
            field: "steam_installed".to_string(),
            path: String::new(),
            message: "Steam installation not found. CrossHook requires Steam to manage Proton and game prefixes.".to_string(),
            remediation: "Install Steam and launch it at least once to create the required directories.".to_string(),
            severity: HealthIssueSeverity::Error,
        });
    } else {
        let path_display = home_to_tilde(&steam_roots[0].to_string_lossy());
        tracing::debug!(path = %path_display, "Steam root found");
        checks.push(HealthIssue {
            field: "steam_installed".to_string(),
            path: path_display,
            message: "Steam installation found.".to_string(),
            remediation: String::new(),
            severity: HealthIssueSeverity::Info,
        });
    }

    // Check 2: Proton available
    if proton_tools.is_empty() {
        checks.push(HealthIssue {
            field: "proton_available".to_string(),
            path: String::new(),
            message: "No Proton installation found. CrossHook requires Proton to run Windows games and trainers.".to_string(),
            remediation: "In Steam, install Proton via Settings → Compatibility, or install GE-Proton via ProtonUp-Qt.".to_string(),
            severity: HealthIssueSeverity::Error,
        });
    } else {
        let path_display = home_to_tilde(&proton_tools[0].path.to_string_lossy());
        checks.push(HealthIssue {
            field: "proton_available".to_string(),
            path: path_display,
            message: format!("{} Proton installation(s) found.", proton_tools.len()),
            remediation: String::new(),
            severity: HealthIssueSeverity::Info,
        });
    }

    // Check 3: Game launched once — scan steamapps/compatdata/*/pfx
    let has_compatdata = steam_roots.iter().any(|root| {
        let compatdata = root.join("steamapps").join("compatdata");
        match fs::read_dir(&compatdata) {
            Ok(entries) => entries
                .filter_map(std::result::Result::ok)
                .any(|entry| entry.path().join("pfx").is_dir()),
            Err(_) => false,
        }
    });

    if !has_compatdata {
        checks.push(HealthIssue {
            field: "game_launched_once".to_string(),
            path: String::new(),
            message: "No Proton game prefix found. Launch a Steam game with Proton at least once to create a prefix.".to_string(),
            remediation: "Enable Proton for a Steam game (Properties → Compatibility) and launch it once.".to_string(),
            severity: HealthIssueSeverity::Warning,
        });
    } else {
        checks.push(HealthIssue {
            field: "game_launched_once".to_string(),
            path: String::new(),
            message: "At least one Proton game prefix found.".to_string(),
            remediation: String::new(),
            severity: HealthIssueSeverity::Info,
        });
    }

    // Check 4: Trainer available — always informational at system check stage
    checks.push(HealthIssue {
        field: "trainer_available".to_string(),
        path: String::new(),
        message: "Trainer availability is verified per-profile when you configure a trainer path.".to_string(),
        remediation: "Download a trainer (.exe) from a trusted source such as FLiNG Trainer and note the file path.".to_string(),
        severity: HealthIssueSeverity::Info,
    });

    // Check 5: umu-run optional launcher
    let mut umu_install_guidance: Option<UmuInstallGuidance> = None;
    {
        match umu_path {
            Some(ref p) => {
                let path_display = home_to_tilde(p);
                tracing::debug!(path = %path_display, "umu-run resolved");
                checks.push(HealthIssue {
                    field: "umu_run_available".to_string(),
                    path: path_display,
                    message:
                        "umu-run found; CrossHook will use it as the preferred Proton launcher."
                            .to_string(),
                    remediation: String::new(),
                    severity: HealthIssueSeverity::Info,
                });
            }
            None if is_flatpak => {
                let distro_family = detect_host_distro_family();
                tracing::debug!(
                    ?distro_family,
                    "umu-run not found on Flatpak host; emitting actionable guidance"
                );
                let advice = build_umu_install_advice(distro_family);
                checks.push(HealthIssue {
                    field: "umu_run_available".to_string(),
                    path: String::new(),
                    message: "umu-run not found in Flatpak host environment; CrossHook will use Proton directly.".to_string(),
                    remediation: advice.remediation,
                    severity: HealthIssueSeverity::Info,
                });
                umu_install_guidance = Some(advice.guidance);
            }
            None => {
                checks.push(HealthIssue {
                    field: "umu_run_available".to_string(),
                    path: String::new(),
                    message: "umu-run not found; CrossHook will use Proton directly. Install umu-launcher for improved runtime bootstrapping.".to_string(),
                    remediation: String::new(),
                    severity: HealthIssueSeverity::Info,
                });
            }
        }
    }

    let critical_failures = checks
        .iter()
        .filter(|c| matches!(c.severity, HealthIssueSeverity::Error))
        .count();
    let warnings = checks
        .iter()
        .filter(|c| matches!(c.severity, HealthIssueSeverity::Warning))
        .count();
    let all_passed = critical_failures == 0 && warnings == 0;

    ReadinessCheckResult {
        checks,
        all_passed,
        critical_failures,
        warnings,
        umu_install_guidance,
    }
}

/// When the user has dismissed the Flatpak umu install reminder, clear the structured
/// guidance payload so readiness stays consistent across reruns and wizard reopen.
pub fn apply_install_nag_dismissal(
    result: &mut ReadinessCheckResult,
    install_nag_dismissed_at: &Option<String>,
) {
    if install_nag_dismissed_at.is_some() {
        result.umu_install_guidance = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::tempdir;

    fn make_proton_exe(dir: &Path) {
        let proton = dir.join("proton");
        fs::write(&proton, b"#!/bin/sh\n").expect("write proton");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&proton).expect("metadata").permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&proton, perms).expect("chmod");
        }
    }

    /// Creates a minimal Steam root with a `steamapps` directory.
    fn setup_steam_root(base: &Path) -> PathBuf {
        let steam_root = base.to_path_buf();
        fs::create_dir_all(steam_root.join("steamapps")).expect("steamapps");
        steam_root
    }

    /// Creates a fake `ProtonInstall` under `steamapps/common/Proton-9`.
    fn make_proton_install(steam_root: &Path) -> ProtonInstall {
        use std::collections::BTreeSet;
        let proton_dir = steam_root.join("steamapps/common/Proton-9");
        fs::create_dir_all(&proton_dir).expect("proton dir");
        make_proton_exe(&proton_dir);
        ProtonInstall {
            name: "Proton-9".to_string(),
            path: proton_dir.join("proton"),
            is_official: true,
            aliases: vec!["Proton-9".to_string()],
            normalized_aliases: BTreeSet::from(["proton9".to_string()]),
        }
    }

    /// Adds a fake compatdata prefix at `steamapps/compatdata/12345/pfx`.
    fn add_compatdata(steam_root: &Path) {
        let pfx = steam_root.join("steamapps/compatdata/12345/pfx");
        fs::create_dir_all(&pfx).expect("pfx dir");
    }

    #[test]
    fn all_pass_case() {
        let tmp = tempdir().expect("tempdir");
        let steam_root = setup_steam_root(tmp.path());
        let proton = make_proton_install(&steam_root);
        add_compatdata(&steam_root);

        let result = evaluate_checks(&[steam_root], &[proton]);

        assert!(
            result.all_passed,
            "expected all_passed; checks: {:?}",
            result.checks
        );
        assert_eq!(result.critical_failures, 0);
        assert_eq!(result.warnings, 0);
        assert_eq!(result.checks.len(), 5);
    }

    #[test]
    fn no_steam_case() {
        let result = evaluate_checks(&[], &[]);

        assert!(!result.all_passed);

        let steam_check = result
            .checks
            .iter()
            .find(|c| c.field == "steam_installed")
            .expect("steam_installed check missing");
        assert!(
            matches!(steam_check.severity, HealthIssueSeverity::Error),
            "expected Error for steam_installed"
        );

        let proton_check = result
            .checks
            .iter()
            .find(|c| c.field == "proton_available")
            .expect("proton_available check missing");
        assert!(
            matches!(proton_check.severity, HealthIssueSeverity::Error),
            "expected Error for proton_available"
        );

        assert_eq!(result.critical_failures, 2);
        assert_eq!(result.warnings, 1); // game_launched_once
    }

    #[test]
    fn no_proton_case() {
        let tmp = tempdir().expect("tempdir");
        let steam_root = setup_steam_root(tmp.path());
        add_compatdata(&steam_root);
        // Intentionally no Proton tools passed

        let result = evaluate_checks(&[steam_root], &[]);

        assert!(!result.all_passed);

        let steam_check = result
            .checks
            .iter()
            .find(|c| c.field == "steam_installed")
            .expect("steam_installed check missing");
        assert!(
            matches!(steam_check.severity, HealthIssueSeverity::Info),
            "expected Info for steam_installed"
        );

        let proton_check = result
            .checks
            .iter()
            .find(|c| c.field == "proton_available")
            .expect("proton_available check missing");
        assert!(
            matches!(proton_check.severity, HealthIssueSeverity::Error),
            "expected Error for proton_available"
        );

        assert_eq!(result.critical_failures, 1);
        assert_eq!(result.warnings, 0);
    }

    #[test]
    fn no_compatdata_case() {
        let tmp = tempdir().expect("tempdir");
        let steam_root = setup_steam_root(tmp.path());
        let proton = make_proton_install(&steam_root);
        // Intentionally no compatdata

        let result = evaluate_checks(&[steam_root], &[proton]);

        assert!(!result.all_passed);
        assert_eq!(result.critical_failures, 0);
        assert_eq!(result.warnings, 1);

        let game_check = result
            .checks
            .iter()
            .find(|c| c.field == "game_launched_once")
            .expect("game_launched_once check missing");
        assert!(
            matches!(game_check.severity, HealthIssueSeverity::Warning),
            "expected Warning for game_launched_once"
        );
    }

    #[test]
    fn native_missing_umu_keeps_info_path() {
        let tmp = tempdir().expect("tempdir");
        let steam_root = setup_steam_root(tmp.path());
        let proton = make_proton_install(&steam_root);
        add_compatdata(&steam_root);

        let result = evaluate_checks_inner(&[steam_root], &[proton], None, false);

        let umu_check = result
            .checks
            .iter()
            .find(|c| c.field == "umu_run_available")
            .expect("umu_run_available check missing");
        assert!(
            matches!(umu_check.severity, HealthIssueSeverity::Info),
            "expected Info severity for native missing umu; got {:?}",
            umu_check.severity
        );
        assert!(
            umu_check.remediation.is_empty(),
            "native missing umu must not emit Flatpak remediation text"
        );
        assert!(
            result.umu_install_guidance.is_none(),
            "native missing umu must not produce a guidance payload"
        );
        // umu being absent must not affect all_passed (Info severity only)
        assert!(
            result.all_passed,
            "missing umu on native must not set all_passed=false; checks: {:?}",
            result.checks
        );
    }

    #[test]
    fn flatpak_missing_umu_reports_actionable_guidance() {
        let tmp = tempdir().expect("tempdir");
        let steam_root = setup_steam_root(tmp.path());
        let proton = make_proton_install(&steam_root);
        add_compatdata(&steam_root);

        let result = evaluate_checks_inner(&[steam_root], &[proton], None, true);

        let umu_check = result
            .checks
            .iter()
            .find(|c| c.field == "umu_run_available")
            .expect("umu_run_available check missing");
        assert!(
            matches!(umu_check.severity, HealthIssueSeverity::Info),
            "Flatpak missing umu must keep Info severity to avoid changing all_passed; got {:?}",
            umu_check.severity
        );
        assert!(
            !umu_check.remediation.is_empty(),
            "Flatpak missing umu must have non-empty remediation text"
        );
        assert!(
            umu_check.message.contains("Flatpak"),
            "Flatpak missing umu message should mention Flatpak host environment"
        );

        let guidance = result
            .umu_install_guidance
            .as_ref()
            .expect("umu_install_guidance payload must be present for Flatpak+missing umu");
        assert!(
            !guidance.install_command.is_empty(),
            "guidance install_command must be non-empty"
        );
        assert!(
            !guidance.docs_url.is_empty(),
            "guidance docs_url must be non-empty"
        );
        assert!(
            !guidance.description.is_empty(),
            "guidance description must be non-empty"
        );

        // umu being absent on Flatpak must not regress all_passed (Info severity only)
        assert!(
            result.all_passed,
            "Flatpak missing umu must not set all_passed=false; checks: {:?}",
            result.checks
        );
    }

    #[test]
    fn detect_host_distro_family_recognizes_arch_like_os_release() {
        let distro = detect_host_distro_family_from_os_release(Some("ID=manjaro\nID_LIKE=arch\n"));
        assert_eq!(distro, HostDistroFamily::Arch);
    }

    #[test]
    fn detect_host_distro_family_recognizes_cachyos_os_release() {
        let distro = detect_host_distro_family_from_os_release(Some("ID=cachyos\nID_LIKE=arch\n"));
        assert_eq!(distro, HostDistroFamily::Arch);
    }

    #[test]
    fn read_host_os_release_uses_host_command_when_flatpak_mount_is_missing() {
        let content = read_host_os_release_with(
            true,
            |_| None,
            || Some("ID=cachyos\nID_LIKE=arch\n".to_string()),
        );

        assert_eq!(content.as_deref(), Some("ID=cachyos\nID_LIKE=arch\n"));
    }

    #[test]
    fn detect_host_distro_family_recognizes_debian_like_os_release() {
        let distro =
            detect_host_distro_family_from_os_release(Some("ID=pop\nID_LIKE=\"ubuntu debian\"\n"));
        assert_eq!(distro, HostDistroFamily::Debian);
    }

    #[test]
    fn build_umu_install_advice_uses_primary_command_per_distro() {
        let arch = build_umu_install_advice(HostDistroFamily::Arch);
        assert_eq!(arch.guidance.install_command, "sudo pacman -S umu-launcher");
        assert!(arch.remediation.contains("source or user-level install"));

        let nix = build_umu_install_advice(HostDistroFamily::Nix);
        assert_eq!(
            nix.guidance.install_command,
            "nix profile install nixpkgs#umu-launcher"
        );
        assert!(nix.remediation.contains("Home Manager"));
    }

    #[test]
    fn install_nag_dismissal_clears_flatpak_umu_guidance() {
        let tmp = tempdir().expect("tempdir");
        let steam_root = setup_steam_root(tmp.path());
        let proton = make_proton_install(&steam_root);
        add_compatdata(&steam_root);

        let mut result = evaluate_checks_inner(&[steam_root], &[proton], None, true);
        assert!(
            result.umu_install_guidance.is_some(),
            "precondition: missing umu on Flatpak must emit guidance"
        );
        apply_install_nag_dismissal(&mut result, &Some("2026-04-15T12:00:00Z".to_string()));
        assert!(
            result.umu_install_guidance.is_none(),
            "dismissed install nag must strip umu_install_guidance on subsequent readiness"
        );
    }
}
