//! umu-launcher install guidance builder, sourced from the readiness catalog.
//!
//! The fallback path emits a single generic command (`pipx install umu-launcher`) when
//! the catalog is absent or the `umu_run` entry carries no command for the detected
//! distro.  Per-distro install commands are authoritative only in the TOML catalog
//! (`default_host_readiness_catalog.toml`) — they must not be duplicated here.

use super::catalog::{global_readiness_catalog, ReadinessCatalog};
use super::{HostDistroFamily, UmuInstallGuidance};

const UMU_LAUNCHER_DOCS_URL: &str = "https://github.com/Open-Wine-Components/umu-launcher";

#[derive(Debug, Clone)]
pub(super) struct UmuInstallAdvice {
    pub(super) guidance: UmuInstallGuidance,
    pub(super) remediation: String,
}

pub(super) fn build_umu_install_advice(distro_family: HostDistroFamily) -> UmuInstallAdvice {
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

    // Generic last-resort fallback: catalog absent or umu_run entry not registered.
    // Do not add per-distro literals here — keep the TOML catalog as the single source.
    let guidance = UmuInstallGuidance {
        install_command: "pipx install umu-launcher".to_string(),
        docs_url: UMU_LAUNCHER_DOCS_URL.to_string(),
        description: "Install umu-launcher on your host to enable improved Proton runtime bootstrapping for non-Steam launches.".to_string(),
    };
    let remediation = format!(
        "Install umu-launcher on your host: `{}`. See {} for full instructions.",
        guidance.install_command, guidance.docs_url,
    );
    UmuInstallAdvice {
        guidance,
        remediation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
