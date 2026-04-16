use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use crate::launch::runtime_helpers::resolve_umu_run_path;
use crate::onboarding::catalog::{global_readiness_catalog, ReadinessCatalog};
use crate::onboarding::{
    HostDistroFamily, HostToolCheckResult, ReadinessCheckResult, UmuInstallGuidance,
};
use crate::profile::health::{HealthIssue, HealthIssueSeverity};
use crate::steam::{discover_compat_tools, discover_steam_root_candidates, ProtonInstall};

const UMU_LAUNCHER_DOCS_URL: &str = "https://github.com/Open-Wine-Components/umu-launcher";

const STEAM_DECK_CAVEATS_DOCS_URL: &str = "https://github.com/ValveSoftware/gamescope/issues";
const STEAM_DECK_CAVEATS_DESCRIPTION: &str =
    "CrossHook works on Steam Deck desktop mode today. In gaming mode you may hit these documented upstream issues on SteamOS 3.7+:";

#[derive(Debug, Clone)]
struct UmuInstallAdvice {
    guidance: UmuInstallGuidance,
    remediation: String,
}

fn read_host_os_release() -> Option<String> {
    crate::platform::read_host_os_release_body()
}

#[cfg(test)]
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

/// Detect host distro family from `/etc/os-release` body (shared with tests).
pub fn detect_host_distro_family_from_os_release(os_release: Option<&str>) -> HostDistroFamily {
    let Some(os_release) = os_release else {
        return HostDistroFamily::Unknown;
    };

    let mut distro_tokens = Vec::new();
    let mut variant_tokens = Vec::new();
    for line in os_release.lines() {
        let Some((key, raw_value)) = line.split_once('=') else {
            continue;
        };
        if key != "ID" && key != "ID_LIKE" && key != "VARIANT_ID" {
            continue;
        }
        let normalized = raw_value
            .trim()
            .trim_matches(|ch| ch == '"' || ch == '\'')
            .to_ascii_lowercase();
        let tokens: Vec<String> = normalized.split_whitespace().map(str::to_string).collect();
        if key == "VARIANT_ID" {
            variant_tokens.extend(tokens);
        } else {
            distro_tokens.extend(tokens);
        }
    }

    // SteamOS / Steam Deck
    if distro_tokens.iter().any(|t| t == "steamos")
        || variant_tokens.iter().any(|t| t == "steamdeck")
    {
        return HostDistroFamily::SteamOS;
    }

    // Gaming-first immutables
    if distro_tokens
        .iter()
        .any(|t| t == "bazzite" || t == "chimeraos")
        || distro_tokens.iter().any(|t| t.contains("universal-blue"))
    {
        return HostDistroFamily::GamingImmutable;
    }

    // Bare immutables (Fedora Atomic family, Vanilla OS, etc.)
    if distro_tokens.iter().any(|t| t == "vanilla")
        || variant_tokens.iter().any(|v| {
            v.contains("kinoite")
                || v.contains("silverblue")
                || v.contains("sericea")
                || v.contains("onyx")
                || v.contains("atomic")
        })
    {
        return HostDistroFamily::BareImmutable;
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

fn detect_host_distro_family() -> HostDistroFamily {
    detect_host_distro_family_from_os_release(read_host_os_release().as_deref())
}

fn build_umu_install_advice(distro_family: HostDistroFamily) -> UmuInstallAdvice {
    let catalog = global_readiness_catalog();
    if let Some(entry) = catalog.find_by_id("umu_run") {
        let install_command = ReadinessCatalog::install_for_distro(entry, distro_family)
            .filter(|cmd| !cmd.command.trim().is_empty())
            .or_else(|| {
                ReadinessCatalog::install_for_distro(entry, HostDistroFamily::Unknown)
                    .filter(|cmd| !cmd.command.trim().is_empty())
            });

        if let Some(cmd) = install_command {
            let guidance = UmuInstallGuidance {
                install_command: cmd.command.clone(),
                docs_url: entry.docs_url.clone(),
                description: entry.description.clone(),
            };
            let alt = if cmd.alternatives.trim().is_empty() {
                String::new()
            } else {
                format!("{} ", cmd.alternatives.trim())
            };
            let remediation = format!(
                "Install umu-launcher on your host: `{}`. {}See {} for full instructions.",
                guidance.install_command, alt, guidance.docs_url,
            );
            return UmuInstallAdvice {
                guidance,
                remediation,
            };
        }
    }

    // Fallback (tests / empty catalog)
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
        HostDistroFamily::SteamOS
        | HostDistroFamily::GamingImmutable
        | HostDistroFamily::BareImmutable => (
            "Install umu-launcher on your host to enable improved Proton runtime bootstrapping for non-Steam launches.",
            "pipx install umu-launcher",
            "See distribution documentation for immutable-friendly install paths.",
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

fn evaluate_checks(
    steam_roots: &[PathBuf],
    proton_tools: &[ProtonInstall],
) -> ReadinessCheckResult {
    let umu_path = resolve_umu_run_path();
    let is_flatpak = crate::platform::is_flatpak();
    let is_steam_deck = crate::platform::is_steam_deck();
    evaluate_checks_inner(
        steam_roots,
        proton_tools,
        umu_path,
        is_flatpak,
        is_steam_deck,
    )
}

fn evaluate_checks_inner(
    steam_roots: &[PathBuf],
    proton_tools: &[ProtonInstall],
    umu_path: Option<String>,
    is_flatpak: bool,
    is_steam_deck: bool,
) -> ReadinessCheckResult {
    let mut checks: Vec<HealthIssue> = Vec::new();

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

    checks.push(HealthIssue {
        field: "trainer_available".to_string(),
        path: String::new(),
        message: "Trainer availability is verified per-profile when you configure a trainer path.".to_string(),
        remediation: "Download a trainer (.exe) from a trusted source such as FLiNG Trainer and note the file path.".to_string(),
        severity: HealthIssueSeverity::Info,
    });

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
                    severity: HealthIssueSeverity::Warning,
                });
                umu_install_guidance = Some(advice.guidance);
            }
            None => {
                checks.push(HealthIssue {
                    field: "umu_run_available".to_string(),
                    path: String::new(),
                    message: "umu-run not found; CrossHook will use Proton directly. Install umu-launcher for improved runtime bootstrapping.".to_string(),
                    remediation: String::new(),
                    severity: HealthIssueSeverity::Warning,
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

    let steam_deck_caveats = if is_steam_deck {
        Some(crate::onboarding::SteamDeckCaveats {
            description: STEAM_DECK_CAVEATS_DESCRIPTION.to_string(),
            items: vec![
                "Black screen until Shader Pre-Caching completes — enable it in Steam Settings → Downloads → Shader Pre-Caching".to_string(),
                "Steam overlay can render below the game under gamescope + Flatpak".to_string(),
                "HDR + gamescope + Flatpak regression on SteamOS 3.7.13 (toggle HDR off if the screen tints or flickers)".to_string(),
            ],
            docs_url: STEAM_DECK_CAVEATS_DOCS_URL.to_string(),
        })
    } else {
        None
    };

    ReadinessCheckResult {
        checks,
        all_passed,
        critical_failures,
        warnings,
        umu_install_guidance,
        steam_deck_caveats,
        tool_checks: Vec::new(),
        detected_distro_family: String::new(),
    }
}

/// Host tool rows from catalog (skips `umu_run` — covered by core umu check).
fn evaluate_host_tool_checks(
    catalog: &ReadinessCatalog,
    distro: HostDistroFamily,
    is_flatpak: bool,
) -> (Vec<HostToolCheckResult>, usize, usize) {
    let mut tool_checks = Vec::new();
    let mut extra_warnings = 0usize;
    let mut extra_critical = 0usize;

    for entry in &catalog.entries {
        if entry.tool_id == "umu_run" {
            continue;
        }
        if entry.binary_name.trim().is_empty() {
            continue;
        }

        let available = crate::platform::host_command_exists(&entry.binary_name);
        if available {
            tool_checks.push(HostToolCheckResult {
                tool_id: entry.tool_id.clone(),
                display_name: entry.display_name.clone(),
                is_available: true,
                is_required: entry.required,
                category: entry.category.clone(),
                docs_url: entry.docs_url.clone(),
                install_guidance: None,
            });
            continue;
        }

        let guidance = if is_flatpak {
            ReadinessCatalog::install_for_distro(entry, distro)
        } else {
            None
        };

        if entry.required {
            extra_critical += 1;
        } else {
            extra_warnings += 1;
        }

        tool_checks.push(HostToolCheckResult {
            tool_id: entry.tool_id.clone(),
            display_name: entry.display_name.clone(),
            is_available: false,
            is_required: entry.required,
            category: entry.category.clone(),
            docs_url: entry.docs_url.clone(),
            install_guidance: guidance,
        });
    }

    (tool_checks, extra_warnings, extra_critical)
}

/// Full readiness: core checks plus catalog host tools.
pub fn check_generalized_readiness(catalog: &ReadinessCatalog) -> ReadinessCheckResult {
    let mut result = check_system_readiness();
    let distro = detect_host_distro_family();
    let is_flatpak = crate::platform::is_flatpak();
    result.detected_distro_family = distro.as_str().to_string();

    let (tool_checks, tw, tc) = evaluate_host_tool_checks(catalog, distro, is_flatpak);
    result.tool_checks = tool_checks;

    result.warnings = result.warnings.saturating_add(tw);
    result.critical_failures = result.critical_failures.saturating_add(tc);
    result.all_passed = result.critical_failures == 0 && result.warnings == 0;

    result
}

/// Clear structured payloads for tool IDs present in `dismissed` (DB-backed nag dismissals).
pub fn apply_readiness_nag_dismissals(
    result: &mut ReadinessCheckResult,
    dismissed: &HashSet<String>,
) {
    if dismissed.contains("umu_run") {
        result.umu_install_guidance = None;
    }
    if dismissed.contains("steam_deck_caveats") {
        result.steam_deck_caveats = None;
    }
    for t in &mut result.tool_checks {
        if dismissed.contains(&t.tool_id) {
            t.install_guidance = None;
        }
    }
}

pub fn apply_install_nag_dismissal(
    result: &mut ReadinessCheckResult,
    install_nag_dismissed_at: &Option<String>,
) {
    if install_nag_dismissed_at.is_some() {
        result.umu_install_guidance = None;
    }
}

pub fn apply_steam_deck_caveats_dismissal(
    result: &mut ReadinessCheckResult,
    dismissed_at: &Option<String>,
) {
    if dismissed_at.is_some() {
        result.steam_deck_caveats = None;
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

    fn setup_steam_root(base: &Path) -> PathBuf {
        let steam_root = base.to_path_buf();
        fs::create_dir_all(steam_root.join("steamapps")).expect("steamapps");
        steam_root
    }

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

        let result = evaluate_checks_inner(
            &[steam_root],
            &[proton],
            Some("umu-run".to_string()),
            false,
            false,
        );

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
        let result = evaluate_checks_inner(&[], &[], None, false, false);

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
        assert_eq!(result.warnings, 2);
    }

    #[test]
    fn no_proton_case() {
        let tmp = tempdir().expect("tempdir");
        let steam_root = setup_steam_root(tmp.path());
        add_compatdata(&steam_root);

        let result = evaluate_checks_inner(&[steam_root], &[], None, false, false);

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
        assert_eq!(result.warnings, 1);
    }

    #[test]
    fn no_compatdata_case() {
        let tmp = tempdir().expect("tempdir");
        let steam_root = setup_steam_root(tmp.path());
        let proton = make_proton_install(&steam_root);

        let result = evaluate_checks_inner(
            &[steam_root],
            &[proton],
            Some("umu-run".to_string()),
            false,
            false,
        );

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

        let result = evaluate_checks_inner(&[steam_root], &[proton], None, false, false);

        let umu_check = result
            .checks
            .iter()
            .find(|c| c.field == "umu_run_available")
            .expect("umu_run_available check missing");
        assert!(
            matches!(umu_check.severity, HealthIssueSeverity::Warning),
            "expected Warning severity for native missing umu; got {:?}",
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
        assert!(
            !result.all_passed,
            "missing umu (Warning) should set all_passed=false; checks: {:?}",
            result.checks
        );
    }

    #[test]
    fn flatpak_missing_umu_reports_actionable_guidance() {
        let _catalog = crate::onboarding::load_readiness_catalog(None);
        crate::onboarding::initialize_readiness_catalog(_catalog);

        let tmp = tempdir().expect("tempdir");
        let steam_root = setup_steam_root(tmp.path());
        let proton = make_proton_install(&steam_root);
        add_compatdata(&steam_root);

        let result = evaluate_checks_inner(&[steam_root], &[proton], None, true, false);

        let umu_check = result
            .checks
            .iter()
            .find(|c| c.field == "umu_run_available")
            .expect("umu_run_available check missing");
        assert!(
            matches!(umu_check.severity, HealthIssueSeverity::Warning),
            "Flatpak missing umu must use Warning severity for amber visual; got {:?}",
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

        assert!(
            !result.all_passed,
            "Flatpak missing umu (Warning) should set all_passed=false; checks: {:?}",
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
    fn detect_host_distro_family_recognizes_steamos() {
        let distro =
            detect_host_distro_family_from_os_release(Some("ID=steamos\nVARIANT_ID=steamdeck\n"));
        assert_eq!(distro, HostDistroFamily::SteamOS);
    }

    #[test]
    fn detect_host_distro_family_recognizes_bazzite() {
        let distro = detect_host_distro_family_from_os_release(Some("ID=bazzite\n"));
        assert_eq!(distro, HostDistroFamily::GamingImmutable);
    }

    #[test]
    fn detect_host_distro_family_recognizes_silverblue() {
        let distro =
            detect_host_distro_family_from_os_release(Some("ID=fedora\nVARIANT_ID=silverblue\n"));
        assert_eq!(distro, HostDistroFamily::BareImmutable);
    }

    #[test]
    fn build_umu_install_advice_uses_primary_command_per_distro() {
        let cat = crate::onboarding::load_readiness_catalog(None);
        crate::onboarding::initialize_readiness_catalog(cat);
        let arch = build_umu_install_advice(HostDistroFamily::Arch);
        assert_eq!(arch.guidance.install_command, "sudo pacman -S umu-launcher");
        assert!(arch.remediation.contains("github.com") || arch.remediation.contains("umu"));

        let nix = build_umu_install_advice(HostDistroFamily::Nix);
        assert_eq!(
            nix.guidance.install_command,
            "nix profile install nixpkgs#umu-launcher"
        );
    }

    #[test]
    fn build_umu_install_advice_skips_empty_catalog_commands() {
        let cat = crate::onboarding::load_readiness_catalog(None);
        crate::onboarding::initialize_readiness_catalog(cat);

        let steam_os = build_umu_install_advice(HostDistroFamily::SteamOS);
        assert_eq!(
            steam_os.guidance.install_command,
            "pipx install umu-launcher"
        );
        assert!(
            steam_os.remediation.contains("pipx install umu-launcher"),
            "SteamOS fallback should stay actionable when the catalog command is blank"
        );

        let gaming_immutable = build_umu_install_advice(HostDistroFamily::GamingImmutable);
        assert_eq!(
            gaming_immutable.guidance.install_command,
            "pipx install umu-launcher"
        );
    }

    #[test]
    fn install_nag_dismissal_clears_flatpak_umu_guidance() {
        let cat = crate::onboarding::load_readiness_catalog(None);
        crate::onboarding::initialize_readiness_catalog(cat);

        let tmp = tempdir().expect("tempdir");
        let steam_root = setup_steam_root(tmp.path());
        let proton = make_proton_install(&steam_root);
        add_compatdata(&steam_root);

        let mut result = evaluate_checks_inner(&[steam_root], &[proton], None, true, false);
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

    #[test]
    fn caveats_absent_when_not_steam_deck() {
        let tmp = tempdir().expect("tempdir");
        let steam_root = setup_steam_root(tmp.path());
        let proton = make_proton_install(&steam_root);
        add_compatdata(&steam_root);

        let result = evaluate_checks_inner(&[steam_root], &[proton], None, false, false);

        assert!(
            result.steam_deck_caveats.is_none(),
            "non-Deck system must not populate steam_deck_caveats"
        );
    }

    #[test]
    fn caveats_present_when_steam_deck_and_not_flatpak() {
        let tmp = tempdir().expect("tempdir");
        let steam_root = setup_steam_root(tmp.path());
        let proton = make_proton_install(&steam_root);
        add_compatdata(&steam_root);

        let result = evaluate_checks_inner(
            &[steam_root],
            &[proton],
            Some("umu-run".to_string()),
            false,
            true,
        );

        let caveats = result
            .steam_deck_caveats
            .as_ref()
            .expect("steam_deck_caveats must be present on Deck (non-Flatpak)");
        assert_eq!(caveats.items.len(), 3, "expected 3 caveat items");
        assert!(
            !caveats.description.is_empty(),
            "caveats description must be non-empty"
        );
        assert!(
            !caveats.docs_url.is_empty(),
            "caveats docs_url must be non-empty"
        );
        assert!(
            result.all_passed,
            "caveats alone must not flip all_passed to false; checks: {:?}",
            result.checks
        );
    }

    #[test]
    fn caveats_present_when_steam_deck_and_flatpak() {
        let tmp = tempdir().expect("tempdir");
        let steam_root = setup_steam_root(tmp.path());
        let proton = make_proton_install(&steam_root);
        add_compatdata(&steam_root);

        let result = evaluate_checks_inner(
            &[steam_root],
            &[proton],
            Some("/usr/bin/umu-run".to_string()),
            true,
            true,
        );

        let caveats = result
            .steam_deck_caveats
            .as_ref()
            .expect("steam_deck_caveats must be present on Deck (Flatpak)");
        assert_eq!(caveats.items.len(), 3, "expected 3 caveat items");
        assert!(
            result.all_passed,
            "caveats alone must not flip all_passed to false; checks: {:?}",
            result.checks
        );
    }

    #[test]
    fn caveats_cleared_after_apply_dismissal() {
        let tmp = tempdir().expect("tempdir");
        let steam_root = setup_steam_root(tmp.path());
        let proton = make_proton_install(&steam_root);
        add_compatdata(&steam_root);

        let mut result = evaluate_checks_inner(&[steam_root], &[proton], None, false, true);
        assert!(
            result.steam_deck_caveats.is_some(),
            "precondition: Deck must populate caveats"
        );

        apply_steam_deck_caveats_dismissal(&mut result, &Some("2026-04-15T12:00:00Z".to_string()));

        assert!(
            result.steam_deck_caveats.is_none(),
            "dismissed caveats must be cleared after apply_steam_deck_caveats_dismissal"
        );
    }

    #[test]
    fn apply_dismissal_noop_when_none() {
        let tmp = tempdir().expect("tempdir");
        let steam_root = setup_steam_root(tmp.path());
        let proton = make_proton_install(&steam_root);
        add_compatdata(&steam_root);

        let mut result = evaluate_checks_inner(&[steam_root], &[proton], None, false, true);
        assert!(
            result.steam_deck_caveats.is_some(),
            "precondition: Deck must populate caveats"
        );

        apply_steam_deck_caveats_dismissal(&mut result, &None);

        assert!(
            result.steam_deck_caveats.is_some(),
            "None dismissed_at must leave caveats intact"
        );
    }

    #[test]
    fn caveats_present_even_when_umu_absent_and_flatpak() {
        let cat = crate::onboarding::load_readiness_catalog(None);
        crate::onboarding::initialize_readiness_catalog(cat);

        let tmp = tempdir().expect("tempdir");
        let steam_root = setup_steam_root(tmp.path());
        let proton = make_proton_install(&steam_root);
        add_compatdata(&steam_root);

        let result = evaluate_checks_inner(&[steam_root], &[proton], None, true, true);

        assert!(
            result.umu_install_guidance.is_some(),
            "Flatpak + missing umu must emit umu_install_guidance"
        );
        assert!(
            result.steam_deck_caveats.is_some(),
            "Deck + Flatpak + missing umu must also emit steam_deck_caveats"
        );
    }

    #[test]
    fn all_passed_stays_true_with_caveats_only() {
        let tmp = tempdir().expect("tempdir");
        let steam_root = setup_steam_root(tmp.path());
        let proton = make_proton_install(&steam_root);
        add_compatdata(&steam_root);

        let result = evaluate_checks_inner(
            &[steam_root],
            &[proton],
            Some("/usr/bin/umu-run".to_string()),
            false,
            true,
        );

        assert!(
            result.steam_deck_caveats.is_some(),
            "Steam Deck caveats must be present"
        );
        assert_eq!(
            result.critical_failures, 0,
            "caveats must not add critical failures"
        );
        assert_eq!(result.warnings, 0, "caveats must not add warnings");
        assert!(
            result.all_passed,
            "all_passed must remain true when only caveats are present; checks: {:?}",
            result.checks
        );
    }
}
