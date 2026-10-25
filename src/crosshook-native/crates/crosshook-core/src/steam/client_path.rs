//! Steam client install root derivation from a Proton compatdata path.
//!
//! Profiles store `steam.compatdata_path` (`<steam root>/steamapps/compatdata/<app id>`),
//! not the Steam client install path, so callers recover the root from it.

const STEAM_COMPATDATA_MARKER: &str = "/steamapps/compatdata/";

/// Returns the Steam client root that owns `compatdata_path`, or an empty string when
/// the path does not contain `/steamapps/compatdata/`.
///
/// Surrounding whitespace is trimmed and `\` separators are normalized to `/` first.
#[must_use]
pub fn steam_client_install_path_from_compatdata(compatdata_path: &str) -> String {
    compatdata_path
        .trim()
        .replace('\\', "/")
        .split_once(STEAM_COMPATDATA_MARKER)
        .map(|(steam_root, _)| steam_root.to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::steam_client_install_path_from_compatdata;

    #[test]
    fn derives_root_before_compatdata_marker() {
        assert_eq!(
            steam_client_install_path_from_compatdata(
                "  /home/user/.local/share/Steam/steamapps/compatdata/1245620  "
            ),
            "/home/user/.local/share/Steam"
        );
    }

    #[test]
    fn normalizes_backslash_separators() {
        assert_eq!(
            steam_client_install_path_from_compatdata(
                r"\home\user\.steam\root\steamapps\compatdata\42"
            ),
            "/home/user/.steam/root"
        );
    }

    #[test]
    fn returns_empty_without_marker() {
        assert_eq!(steam_client_install_path_from_compatdata(""), "");
        assert_eq!(
            steam_client_install_path_from_compatdata("/prefixes/alpha/pfx"),
            ""
        );
    }
}
