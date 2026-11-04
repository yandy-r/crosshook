use std::fs;

use crate::profile::ProfileStore;

use super::{redact_home_paths, secrets::redact_secret_settings};

/// Reads each profile TOML as raw text, optionally redacting home paths.
pub(super) fn collect_profiles(store: &ProfileStore, redact_paths: bool) -> Vec<(String, String)> {
    let names = match store.list() {
        Ok(names) => names,
        Err(_) => return Vec::new(),
    };

    let mut profiles = Vec::with_capacity(names.len());
    for name in &names {
        let path = store.base_path.join(format!("{name}.toml"));
        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(_) => continue,
        };
        let content = if redact_paths {
            redact_home_paths(&content)
        } else {
            content
        };
        profiles.push((name.clone(), content));
    }

    profiles
}

/// Sanitizes the raw settings TOML (`None` when the file could not be read):
/// redacts credentials and omits the private recent-palette identifiers.
/// Fails closed: if the file cannot be parsed or re-serialized, no raw
/// contents are emitted.
pub(super) fn collect_settings(content: Option<&str>, redact_paths: bool) -> String {
    let Some(content) = content else {
        return "(settings file not found)".to_string();
    };
    let content = match content.parse::<toml::Table>() {
        Ok(mut settings) => {
            redact_secret_settings(&mut settings);
            if let Some(ui) = settings.get_mut("ui").and_then(toml::Value::as_table_mut) {
                ui.remove("palette_recent");
            }
            toml::to_string(&settings).unwrap_or_else(|_| {
                "(settings file could not be serialized; contents omitted)".to_string()
            })
        }
        Err(_) => "(settings file could not be parsed; contents omitted)".to_string(),
    };
    if redact_paths {
        redact_home_paths(&content)
    } else {
        content
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use tempfile::tempdir;

    #[test]
    fn collect_profiles_returns_correct_count() {
        let temp = tempdir().unwrap();
        let profiles_dir = temp.path().join("profiles");
        fs::create_dir_all(&profiles_dir).unwrap();
        fs::write(
            profiles_dir.join("game-a.toml"),
            "[game]\nname = \"Game A\"\n",
        )
        .unwrap();
        fs::write(
            profiles_dir.join("game-b.toml"),
            "[game]\nname = \"Game B\"\n",
        )
        .unwrap();

        let store = ProfileStore::with_base_path(profiles_dir);
        let profiles = collect_profiles(&store, false);
        assert_eq!(profiles.len(), 2);
    }

    #[test]
    fn collect_profiles_with_redaction_replaces_home_paths() {
        let home = env::var("HOME").unwrap();
        let temp = tempdir().unwrap();
        let profiles_dir = temp.path().join("profiles");
        fs::create_dir_all(&profiles_dir).unwrap();
        fs::write(
            profiles_dir.join("test.toml"),
            format!("[game]\nname = \"Test\"\nexecutable_path = \"{home}/games/test.exe\"\n"),
        )
        .unwrap();

        let store = ProfileStore::with_base_path(profiles_dir);
        let profiles = collect_profiles(&store, true);
        assert_eq!(profiles.len(), 1);
        let (_, content) = &profiles[0];
        assert!(content.contains("~/games/test.exe"));
        assert!(!content.contains(&home));
    }

    #[test]
    fn collect_settings_excludes_only_recent_palette_ids() {
        let content = collect_settings(
            Some(
                "last_used_profile = \"daily\"\n\
                 [ui]\n\
                 library_view_mode = \"list\"\n\
                 palette_recent = [\"private-palette-id\"]\n\
                 [ui.extra]\n\
                 custom_palette = \"keep-me\"\n",
            ),
            false,
        );

        assert!(!content.contains("palette_recent"));
        assert!(!content.contains("private-palette-id"));
        assert!(content.contains("last_used_profile = \"daily\""));
        assert!(content.contains("library_view_mode = \"list\""));
        assert!(content.contains("custom_palette = \"keep-me\""));
    }

    #[test]
    fn collect_settings_redacts_api_key_value() {
        let content = collect_settings(
            Some("steamgriddb_api_key = \"sgdb-test-key-0123456789abcdef\"\n"),
            false,
        );
        assert!(!content.contains("sgdb-test-key-0123456789abcdef"));
        assert!(content.contains("steamgriddb_api_key = \"<redacted>\""));
    }

    #[test]
    fn collect_settings_omits_malformed_file_with_private_palette_ids() {
        let malformed = "[ui]\npalette_recent = [\"private-palette-id\"]\ninvalid = [";
        for redact_paths in [false, true] {
            assert_eq!(
                collect_settings(Some(malformed), redact_paths),
                "(settings file could not be parsed; contents omitted)"
            );
        }
    }
}
