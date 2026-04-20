use crate::settings::SettingsStore;

use super::redact_home_paths;

pub(super) fn collect_crosshook_info(settings_store: &SettingsStore, redact_paths: bool) -> String {
    let mut lines = Vec::new();

    lines.push(format!("CrossHook version: {}", env!("CARGO_PKG_VERSION")));
    lines.push(format!(
        "Settings path: {}",
        settings_store.settings_path().display()
    ));
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
            lines.push(format!("(failed to load settings: {error})"));
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
}
