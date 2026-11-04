//! Bundle-level tests that no credential leaves the machine.

use std::fs;

use tempfile::tempdir;

use super::tests::{entry, export_entries, read_entries};
use super::*;
use crate::profile::ProfileStore;
use crate::settings::SettingsStore;

/// Exports a bundle of `profiles` (`(file name, contents)`) next to an empty
/// settings store, with `ui_log` as an extra file, and returns its entries.
fn export_profiles(profiles: &[(&str, &str)], ui_log: &str) -> Vec<(String, String)> {
    let temp = tempdir().unwrap();
    let profiles_dir = temp.path().join("profiles");
    fs::create_dir_all(&profiles_dir).unwrap();
    for (name, contents) in profiles {
        fs::write(profiles_dir.join(name), contents).unwrap();
    }
    let options = DiagnosticBundleOptions {
        output_dir: Some(temp.path().join("output")),
        extra_files: vec![DiagnosticBundleFile {
            name: "ui.log".to_string(),
            contents: ui_log.as_bytes().to_vec(),
        }],
        ..Default::default()
    };
    let result = export_diagnostic_bundle(
        &ProfileStore::with_base_path(profiles_dir),
        &SettingsStore::with_base_path(temp.path().join("settings")),
        &options,
    )
    .unwrap();
    read_entries(&result.archive_path)
}

fn assert_absent(entries: &[(String, String)], secret: &str) {
    for (path, text) in entries {
        assert!(!text.contains(secret), "{path} leaks {secret:?}:\n{text}");
    }
}

const TAP_TOKEN: &str = "ghp-tap-token-0123456789abcdef";

#[test]
fn export_strips_tap_url_credentials_from_every_entry() {
    let tap_url = format!("https://reader:{TAP_TOKEN}@github.com/org/taps.git");
    let settings = format!("[[community_taps]]\nurl = \"{tap_url}\"\nbranch = \"main\"\n");
    let extra_files = vec![DiagnosticBundleFile {
        name: "ui.log".to_string(),
        contents: format!("git fetch {tap_url} failed\nauth with {TAP_TOKEN} rejected\n")
            .into_bytes(),
    }];

    for redact_paths in [false, true] {
        let entries = export_entries(&settings, redact_paths, extra_files.clone());

        for (path, text) in &entries {
            assert!(!text.contains(TAP_TOKEN), "{path} leaks the tap token");
            assert!(!text.contains("reader:"), "{path} leaks the tap user");
        }
        assert!(
            entry(&entries, "/crosshook-info.txt").contains("  - https://github.com/org/taps.git")
        );
        assert!(
            entry(&entries, "/settings.toml").contains("url = \"https://github.com/org/taps.git\"")
        );
        assert_eq!(
            entry(&entries, "/app/ui.log"),
            "git fetch https://<redacted>github.com/org/taps.git failed\nauth with <redacted> rejected\n"
        );
    }
}

const ENV_SECRET: &str = "env-secret-value-0123456789";

#[test]
fn export_masks_secret_custom_env_vars_in_profiles() {
    let temp = tempdir().unwrap();
    let profiles_dir = temp.path().join("profiles");
    fs::create_dir_all(&profiles_dir).unwrap();
    fs::write(
        profiles_dir.join("game.toml"),
        format!(
            "[game]\nname = \"Game\"\n\
             [launch.custom_env_vars]\nAPI_TOKEN = \"{ENV_SECRET}\"\nDXVK_ASYNC = \"1\"\n\
             [local_override.launch.custom_env_vars]\nmy_password = \"{ENV_SECRET}-2\"\n"
        ),
    )
    .unwrap();
    fs::write(
        profiles_dir.join("broken.toml"),
        format!("[launch.custom_env_vars]\nSECRET_KEY = {ENV_SECRET}\n"),
    )
    .unwrap();
    let settings_store = SettingsStore::with_base_path(temp.path().join("settings"));
    let options = DiagnosticBundleOptions {
        output_dir: Some(temp.path().join("output")),
        extra_files: vec![DiagnosticBundleFile {
            name: "ui.log".to_string(),
            contents: format!("env API_TOKEN={ENV_SECRET}").into_bytes(),
        }],
        ..Default::default()
    };

    let result = export_diagnostic_bundle(
        &ProfileStore::with_base_path(profiles_dir),
        &settings_store,
        &options,
    )
    .unwrap();
    let entries = read_entries(&result.archive_path);

    for (path, text) in &entries {
        assert!(!text.contains(ENV_SECRET), "{path} leaks a custom env var");
    }
    let game = entry(&entries, "/profiles/game.toml");
    assert!(game.contains("API_TOKEN = \"<redacted>\""), "{game}");
    assert!(game.contains("DXVK_ASYNC = \"1\""), "{game}");
    assert!(game.contains("my_password = \"<redacted>\""), "{game}");
    assert_eq!(
        entry(&entries, "/profiles/broken.toml"),
        "(profile file could not be parsed; contents omitted)"
    );
    assert_eq!(entry(&entries, "/app/ui.log"), "env API_TOKEN=<redacted>");
}

#[test]
fn export_skips_settings_and_recent_files_in_profiles_dir() {
    let temp = tempdir().unwrap();
    let config_dir = temp.path().join("config");
    fs::create_dir_all(&config_dir).unwrap();
    let settings_store = SettingsStore::with_base_path(config_dir.clone());
    fs::write(
        settings_store.settings_path(),
        "last_used_profile = \"game\"\n",
    )
    .unwrap();
    fs::write(
        config_dir.join("recent.toml"),
        "game_paths = [\"/games/private.exe\"]\n",
    )
    .unwrap();
    fs::write(config_dir.join("game.toml"), "[game]\nname = \"Game\"\n").unwrap();
    // Profiles reach the same directory through a symlink.
    let profiles_link = temp.path().join("profiles");
    std::os::unix::fs::symlink(&config_dir, &profiles_link).unwrap();
    let options = DiagnosticBundleOptions {
        output_dir: Some(temp.path().join("output")),
        recent_files_path: Some(config_dir.join("recent.toml")),
        ..Default::default()
    };

    let result = export_diagnostic_bundle(
        &ProfileStore::with_base_path(profiles_link),
        &settings_store,
        &options,
    )
    .unwrap();
    let entries = read_entries(&result.archive_path);

    let profile_entries: Vec<&str> = entries
        .iter()
        .filter_map(|(path, _)| path.split_once("/profiles/").map(|(_, name)| name))
        .collect();
    assert_eq!(profile_entries, ["game.toml"]);
    assert_eq!(result.summary.profile_count, 1);
    let health: serde_json::Value =
        serde_json::from_str(entry(&entries, "/health-summary.json")).unwrap();
    assert_eq!(health["total_count"], 1, "{health}");
    assert_eq!(health["profiles"][0]["name"], "game", "{health}");
    for (path, text) in &entries {
        assert!(
            !text.contains("/games/private.exe"),
            "{path} leaks recent files"
        );
    }
}

#[test]
fn health_summary_hides_env_secrets_of_profiles_that_fail_to_load() {
    let entries = export_profiles(
        &[
            // Unparseable: the secret sits in an inline table.
            (
                "inline.toml",
                "[launch]\ncustom_env_vars = { API_TOKEN = inlineSecret0123 }\n",
            ),
            // Parses as TOML, fails typed loading; JSON escapes the quotes.
            (
                "array.toml",
                "[launch.custom_env_vars]\nAPI_TOKEN = [\"arraySecret0123\"]\n",
            ),
            // Shorter than the free-text minimum.
            (
                "short.toml",
                "[launch.custom_env_vars]\nDB_PASSWORD = 7654321\n",
            ),
            // The parse error points at the line holding the secret.
            (
                "triple.toml",
                "[launch.custom_env_vars]\nAPI_TOKEN = \"\"\"tripleSecret0123\"\"\" x\n",
            ),
            (
                "escaped.toml",
                "[launch.custom_env_vars]\nAPI_TOKEN = \"esc\\\"apedSecret0123\" x\n",
            ),
        ],
        "",
    );

    for secret in [
        "inlineSecret0123",
        "arraySecret0123",
        "7654321",
        "tripleSecret0123",
        "apedSecret0123",
    ] {
        assert_absent(&entries, secret);
    }
    let health: serde_json::Value =
        serde_json::from_str(entry(&entries, "/health-summary.json")).unwrap();
    let messages: Vec<&str> = health["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|report| report["issues"][0]["message"].as_str().unwrap())
        .collect();
    assert_eq!(messages.len(), 5, "{health}");
    for message in messages {
        assert!(
            message.starts_with("Profile could not be loaded: TOML parse error at line "),
            "{message}"
        );
        assert!(!message.contains('\n'), "{message}");
    }
}

#[test]
fn short_profile_secrets_do_not_mangle_health_messages() {
    let entries = export_profiles(
        &[(
            "short.toml",
            "[game]\nname = \"Game 1\"\nexecutable_path = \"/games/v1.1/game1.exe\"\n\
             [launch.custom_env_vars]\nDB_PASSWORD = \"1\"\nSDL_KEYBOARD_GRAB = \"1\"\n",
        )],
        "",
    );

    let health = entry(&entries, "/health-summary.json");
    assert!(health.contains("/games/v1.1/game1.exe"), "{health}");
    assert!(!health.contains("<redacted>"), "{health}");
    let profile = entry(&entries, "/profiles/short.toml");
    assert!(
        profile.contains("DB_PASSWORD = \"<redacted>\""),
        "{profile}"
    );
    assert!(profile.contains("SDL_KEYBOARD_GRAB = \"1\""), "{profile}");
}

#[test]
fn non_table_custom_env_vars_value_is_redacted_whole() {
    let entries = export_profiles(
        &[(
            "str.toml",
            "[launch]\ncustom_env_vars = \"API_TOKEN=stringSecret0123\"\n",
        )],
        "",
    );

    assert_absent(&entries, "stringSecret0123");
    let profile = entry(&entries, "/profiles/str.toml");
    assert!(
        profile.contains("custom_env_vars = \"<redacted>\""),
        "{profile}"
    );
}

#[test]
fn proxy_credentials_in_ordinary_profile_values_are_stripped() {
    let entries = export_profiles(
        &[(
            "proxy.toml",
            "[launch.custom_env_vars]\nHTTP_PROXY = \"http://user:proxyPw0123456@proxy:3128\"\n",
        )],
        "proxy auth proxyPw0123456 rejected\n",
    );

    assert_absent(&entries, "proxyPw0123456");
    let profile = entry(&entries, "/profiles/proxy.toml");
    assert!(
        profile.contains("HTTP_PROXY = \"http://proxy:3128\""),
        "{profile}"
    );
}

#[test]
fn tap_credentials_are_scrubbed_from_slugified_tap_folder_names() {
    let settings = "[[community_taps]]\n\
                    url = \"https://reader:ghp_TapTok3n0123456789@github.com/org/taps.git\"\n";
    let ui_log = "failed to read /data/community/taps/\
                  https-reader-ghp-taptok3n0123456789-github-com-org-taps-git/index.json\n";
    let entries = export_entries(
        settings,
        false,
        vec![DiagnosticBundleFile {
            name: "ui.log".to_string(),
            contents: ui_log.as_bytes().to_vec(),
        }],
    );

    assert_absent(&entries, "taptok3n0123456789");
    assert_absent(&entries, "TapTok3n0123456789");
}

#[test]
fn tap_credentials_with_quote_or_percent_encoding_are_removed() {
    let settings = "[[community_taps]]\nurl = \"https://u:pa'ss0123456789@host/a.git\"\n\
                    [[community_taps]]\nurl = \"https://u:p%40ss9876543210@host/b.git\"\n";
    let ui_log = "auth p@ss9876543210 failed\n";
    let entries = export_entries(
        settings,
        false,
        vec![DiagnosticBundleFile {
            name: "ui.log".to_string(),
            contents: ui_log.as_bytes().to_vec(),
        }],
    );

    for secret in ["pa'ss0123456789", "p%40ss9876543210", "p@ss9876543210"] {
        assert_absent(&entries, secret);
    }
    let info = entry(&entries, "/crosshook-info.txt");
    assert!(info.contains("  - https://host/a.git"), "{info}");
}

#[test]
fn hardlinked_settings_file_is_not_exported_as_a_profile() {
    let temp = tempdir().unwrap();
    let settings_store = SettingsStore::with_base_path(temp.path().join("config"));
    fs::create_dir_all(&settings_store.base_path).unwrap();
    fs::write(
        settings_store.settings_path(),
        "[ui]\npalette_recent = [\"private-palette-id\"]\n",
    )
    .unwrap();
    let profiles_dir = temp.path().join("profiles");
    fs::create_dir_all(&profiles_dir).unwrap();
    fs::hard_link(
        settings_store.settings_path(),
        profiles_dir.join("linked.toml"),
    )
    .unwrap();
    fs::write(profiles_dir.join("game.toml"), "[game]\nname = \"Game\"\n").unwrap();
    let options = DiagnosticBundleOptions {
        output_dir: Some(temp.path().join("output")),
        ..Default::default()
    };

    let result = export_diagnostic_bundle(
        &ProfileStore::with_base_path(profiles_dir),
        &settings_store,
        &options,
    )
    .unwrap();
    let entries = read_entries(&result.archive_path);

    assert_absent(&entries, "private-palette-id");
    assert_eq!(result.summary.profile_count, 1);
}
