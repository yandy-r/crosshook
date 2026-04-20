use std::fs;
use std::path::PathBuf;

use crate::launch::runtime_helpers::resolve_umu_run_path;
use crate::onboarding::{ReadinessCheckResult, UmuInstallGuidance};
use crate::profile::health::{HealthIssue, HealthIssueSeverity};
use crate::steam::{discover_compat_tools, discover_steam_root_candidates, ProtonInstall};

use super::super::distro::detect_host_distro_family;
use super::super::install_advice::build_umu_install_advice;

const STEAM_DECK_CAVEATS_DOCS_URL: &str = "https://github.com/ValveSoftware/gamescope/issues";
const STEAM_DECK_CAVEATS_DESCRIPTION: &str =
    "CrossHook works on Steam Deck desktop mode today. In gaming mode you may hit these documented upstream issues on SteamOS 3.7+:";

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

pub(super) fn evaluate_checks_inner(
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
