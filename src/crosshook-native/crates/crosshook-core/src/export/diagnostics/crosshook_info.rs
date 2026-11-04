use crate::settings::{SettingsStore, SettingsStoreError};

use super::redact_home_paths;

pub(super) fn collect_crosshook_info(settings_store: &SettingsStore, redact_paths: bool) -> String {
    let mut lines = Vec::new();

    lines.push(format!("CrossHook version: {}", env!("CARGO_PKG_VERSION")));
    let settings_path = settings_store.settings_path().display().to_string();
    let settings_path = if redact_paths {
        redact_home_paths(&settings_path)
    } else {
        settings_path
    };
    lines.push(format!("Settings path: {settings_path}"));
    lines.push(String::new());

    match settings_store.load() {
        Ok(data) => {
            lines.push(format!(
                "auto_load_last_profile: {}",
                data.auto_load_last_profile
            ));
            lines.push(format!("last_used_profile: {}", data.last_used_profile));
            lines.push(format!("community_taps: {}", data.community_taps.len()));
            for tap in &data.community_taps {
                let url = if redact_paths {
                    redact_home_paths(&tap.url)
                } else {
                    tap.url.clone()
                };
                lines.push(format!("  - {url}"));
            }
        }
        Err(error) => {
            // The TOML error text quotes the offending source line, which may hold a credential.
            let reason = match error {
                SettingsStoreError::Io(_) => "I/O error",
                SettingsStoreError::TomlDe(_) => "TOML syntax or type error",
                SettingsStoreError::TomlSer(_) => "TOML serialization error",
            };
            lines.push(format!("(failed to load settings: {reason})"));
        }
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn collect_crosshook_info_includes_version() {
        let temp = tempdir().unwrap();
        let store = SettingsStore::with_base_path(temp.path().to_path_buf());
        let info = collect_crosshook_info(&store, false);
        assert!(info.contains(env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn collect_crosshook_info_omits_settings_parse_error_text() {
        const KEY: &str = "sgdb-test-key-0123456789abcdef";
        let temp = tempdir().unwrap();
        let store = SettingsStore::with_base_path(temp.path().to_path_buf());
        std::fs::write(
            store.settings_path(),
            format!("steamgriddb_api_key = {KEY}\n"),
        )
        .unwrap();

        let info = collect_crosshook_info(&store, false);

        assert!(!info.contains(KEY));
        assert!(info.contains("(failed to load settings: TOML syntax or type error)"));
    }
}
