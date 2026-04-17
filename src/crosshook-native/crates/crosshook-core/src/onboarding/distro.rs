//! Host distribution family detection from `/etc/os-release`.

use super::HostDistroFamily;

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

pub(super) fn detect_host_distro_family() -> HostDistroFamily {
    detect_host_distro_family_from_os_release(read_host_os_release().as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
