use std::fs;

use crate::profile::ProfileStore;
use crate::settings::SettingsStore;

use super::redact_home_paths;

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

/// Reads the settings TOML file as raw text.
pub(super) fn collect_settings(settings_store: &SettingsStore, redact_paths: bool) -> String {
    let path = settings_store.settings_path();
    match fs::read_to_string(&path) {
        Ok(content) => {
            if redact_paths {
                redact_home_paths(&content)
            } else {
                content
            }
        }
        Err(_) => "(settings file not found)".to_string(),
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
}
