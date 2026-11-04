use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use crate::profile::ProfileStore;

use super::redact_home_paths;
use super::secrets::{
    is_secret_env_name, redact_secret_env_vars, redact_secret_settings, scan_secret_lines,
};
use super::url_credentials::{
    strip_url_userinfo, structured_userinfos, userinfo_secrets, userinfos_in,
};

/// Placeholder exported in place of a profile that cannot be sanitized.
const UNPARSEABLE_PROFILE: &str = "(profile file could not be parsed; contents omitted)";

/// Profiles prepared for the bundle.
#[derive(Debug, Default)]
pub(super) struct CollectedProfiles {
    /// `(name, sanitized TOML)` pairs.
    pub(super) entries: Vec<(String, String)>,
    /// Credential values removed from any profile, for the bundle scrubber.
    pub(super) secrets: Vec<String>,
    /// Names skipped because they are the settings or recent-files TOML.
    pub(super) excluded_names: Vec<String>,
}

/// Reads each profile TOML, masks secret-named `custom_env_vars` values and
/// URL credentials, and optionally redacts home paths. Files that are the
/// same file as an `excluded` path (the settings and recent-files TOML when
/// they share the profiles directory, through any symlink or hard link) are
/// skipped. Fails closed: a profile that does not parse is replaced by a
/// placeholder.
pub(super) fn collect_profiles(
    store: &ProfileStore,
    excluded: &[PathBuf],
    redact_paths: bool,
) -> CollectedProfiles {
    let Ok(names) = store.list() else {
        return CollectedProfiles::default();
    };
    let excluded: Vec<FileId> = excluded.iter().filter_map(|path| file_id(path)).collect();

    let mut collected = CollectedProfiles::default();
    for name in names {
        let path = store.base_path.join(format!("{name}.toml"));
        if file_id(&path).is_some_and(|id| excluded.contains(&id)) {
            collected.excluded_names.push(name);
            continue;
        }
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        let content = sanitize_profile(&content, &mut collected.secrets);
        let content = if redact_paths {
            redact_home_paths(&content)
        } else {
            content
        };
        collected.entries.push((name, content));
    }
    collected
}

/// `(st_dev, st_ino)` of the file at `path`, following symlinks.
type FileId = (u64, u64);

fn file_id(path: &Path) -> Option<FileId> {
    fs::metadata(path).ok().map(|meta| (meta.dev(), meta.ino()))
}

/// Masks secret environment variables and URL credentials in a profile,
/// appending the removed values to `secrets`. Unparseable content yields
/// [`UNPARSEABLE_PROFILE`]; its secret-looking values are still reported,
/// since a health-check error can echo them.
fn sanitize_profile(content: &str, secrets: &mut Vec<String>) -> String {
    let Ok(mut profile) = content.parse::<toml::Table>() else {
        secrets.extend(scan_secret_lines(content, is_secret_env_name));
        secrets.extend(userinfos_in(content).into_iter().flat_map(userinfo_secrets));
        return UNPARSEABLE_PROFILE.to_string();
    };
    redact_secret_env_vars(&mut profile, secrets);
    strip_table_url_userinfo(&mut profile, secrets);
    toml::to_string(&profile).unwrap_or_else(|_| UNPARSEABLE_PROFILE.to_string())
}

/// Removes the userinfo of every URL in every string value of `table`, at
/// any depth, appending the removed credentials to `secrets`.
fn strip_table_url_userinfo(table: &mut toml::Table, secrets: &mut Vec<String>) {
    for (_, value) in table.iter_mut() {
        strip_value_url_userinfo(value, secrets);
    }
}

fn strip_value_url_userinfo(value: &mut toml::Value, secrets: &mut Vec<String>) {
    match value {
        toml::Value::String(text) => {
            let userinfos = structured_userinfos(text);
            if userinfos.is_empty() {
                return;
            }
            secrets.extend(userinfos.into_iter().flat_map(userinfo_secrets));
            *text = strip_url_userinfo(text).into_owned();
        }
        toml::Value::Array(items) => {
            for item in items {
                strip_value_url_userinfo(item, secrets);
            }
        }
        toml::Value::Table(inner) => strip_table_url_userinfo(inner, secrets),
        _ => {}
    }
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
            strip_tap_url_userinfo(&mut settings);
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

/// Removes credentials from every `community_taps[*].url`.
fn strip_tap_url_userinfo(settings: &mut toml::Table) {
    let Some(taps) = settings
        .get_mut("community_taps")
        .and_then(toml::Value::as_array_mut)
    else {
        return;
    };
    for url in taps.iter_mut().filter_map(|tap| tap.get_mut("url")) {
        if let Some(stripped) = url
            .as_str()
            .map(|text| strip_url_userinfo(text).into_owned())
        {
            *url = toml::Value::String(stripped);
        }
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
        let profiles = collect_profiles(&store, &[], false);
        assert_eq!(profiles.entries.len(), 2);
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
        let profiles = collect_profiles(&store, &[], true);
        assert_eq!(profiles.entries.len(), 1);
        let (_, content) = &profiles.entries[0];
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

    #[test]
    fn collect_profiles_skips_excluded_paths() {
        let temp = tempdir().unwrap();
        fs::write(temp.path().join("game.toml"), "[game]\nname = \"Game\"\n").unwrap();
        fs::write(temp.path().join("settings.toml"), "offline_mode = true\n").unwrap();

        let store = ProfileStore::with_base_path(temp.path().to_path_buf());
        let excluded = [
            temp.path().join("settings.toml"),
            temp.path().join("missing.toml"),
        ];
        let profiles = collect_profiles(&store, &excluded, false);

        let names: Vec<&str> = profiles
            .entries
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        assert_eq!(names, ["game"]);
    }

    #[test]
    fn collect_settings_strips_tap_url_userinfo() {
        let content = collect_settings(
            Some(
                "[[community_taps]]\nurl = \"https://bot:ghp-token-0123@github.com/o/r.git\"\n\
                 [[community_taps]]\nurl = \"git@github.com:o/r.git\"\nbranch = \"main\"\n",
            ),
            false,
        );

        assert!(!content.contains("ghp-token-0123"), "{content}");
        assert!(
            content.contains("url = \"https://github.com/o/r.git\""),
            "{content}"
        );
        assert!(
            content.contains("url = \"git@github.com:o/r.git\""),
            "{content}"
        );
    }
}
