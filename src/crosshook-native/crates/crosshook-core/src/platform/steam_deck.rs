use super::detect::is_flatpak;
use super::gateway::host_std_command;

/// Reads `/etc/os-release` from the host namespace when possible: tries
/// `/run/host/etc/os-release` first, then on Flatpak uses [`host_std_command`] to
/// run `cat /etc/os-release` on the host when the bind-mount is missing (avoids
/// reading the sandbox copy), otherwise reads `/etc/os-release` in the current
/// mount namespace. Shared with onboarding distro detection.
pub(crate) fn read_host_os_release_body() -> Option<String> {
    if let Ok(content) = std::fs::read_to_string("/run/host/etc/os-release") {
        return Some(content);
    }
    if is_flatpak() {
        return host_std_command("cat")
            .arg("/etc/os-release")
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok());
    }
    std::fs::read_to_string("/etc/os-release").ok()
}

/// Returns `true` when running on a Steam Deck (SteamOS).
///
/// Detection tries multiple signals in order:
/// - `SteamDeck=1` environment variable (set by SteamOS session)
/// - `SteamOS=1` environment variable (set by SteamOS session)
/// - `VARIANT_ID=steamdeck` or `ID=steamos` in the host os-release content from
///   [`read_host_os_release_body`] (covers `/run/host/…`, Flatpak host `cat`, or native `/etc/os-release`)
///
/// The first signal that fires wins; the function short-circuits on env vars
/// before touching the filesystem.
pub fn is_steam_deck() -> bool {
    is_steam_deck_from_sources(
        |key| std::env::var(key).ok(),
        read_host_os_release_body().as_deref(),
    )
}

pub(crate) fn is_steam_deck_from_sources(
    env_lookup: impl Fn(&str) -> Option<String>,
    os_release: Option<&str>,
) -> bool {
    if env_lookup("SteamDeck").as_deref() == Some("1") {
        return true;
    }
    if env_lookup("SteamOS").as_deref() == Some("1") {
        return true;
    }
    let Some(body) = os_release else {
        return false;
    };
    for line in body.lines() {
        let Some((key, raw_value)) = line.split_once('=') else {
            continue;
        };
        let value = raw_value
            .trim()
            .trim_matches(|ch: char| ch == '"' || ch == '\'');
        match key {
            "VARIANT_ID" if value.eq_ignore_ascii_case("steamdeck") => return true,
            "ID" if value == "steamos" => return true,
            _ => {}
        }
    }
    false
}
