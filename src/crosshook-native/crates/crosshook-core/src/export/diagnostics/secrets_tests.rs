//! Tests for diagnostic-bundle credential handling.

use super::*;

const KEY: &str = "sgdb-test-key-0123456789abcdef";

fn scrubber_for(settings: &str) -> SecretScrubber {
    SecretScrubber::new(settings_secrets(settings))
}

fn scrub_str(scrubber: &SecretScrubber, text: &str) -> String {
    String::from_utf8(scrubber.scrub(text.as_bytes()).into_owned()).unwrap()
}

#[test]
fn redact_secret_settings_keeps_empty_value_distinguishable() {
    let mut table: toml::Table = "steamgriddb_api_key = \"\"".parse().unwrap();
    redact_secret_settings(&mut table);
    assert_eq!(table["steamgriddb_api_key"].as_str(), Some(""));

    let mut table: toml::Table = "steamgriddb_api_key = 42".parse().unwrap();
    redact_secret_settings(&mut table);
    assert_eq!(table["steamgriddb_api_key"].as_str(), Some(REDACTED_MARKER));
}

#[test]
fn line_scan_extracts_value_in_every_spelling() {
    for line in [
        format!("steamgriddb_api_key = \"{KEY}\" # note"),
        format!("steamgriddb_api_key = '{KEY}'"),
        format!("steamgriddb_api_key = {KEY} # note"),
        format!("\"steamgriddb_api_key\" = \"{KEY}"),
    ] {
        let content = format!("{line}\nbroken = [\n");
        let scrubber = scrubber_for(&content);
        assert_eq!(
            scrub_str(&scrubber, &format!("x {KEY} y")),
            "x <redacted> y",
            "{line}"
        );
    }
}

#[test]
fn scrub_replaces_adjacent_and_boundary_matches_and_borrows_without_match() {
    let scrubber = scrubber_for(&format!("steamgriddb_api_key = \"{KEY}\""));
    assert_eq!(
        scrub_str(&scrubber, &format!("{KEY}{KEY} mid {KEY}")),
        "<redacted><redacted> mid <redacted>"
    );
    assert!(matches!(scrubber.scrub(b"short"), Cow::Borrowed(_)));
    assert!(matches!(
        scrubber.scrub(b"no secret here"),
        Cow::Borrowed(_)
    ));
}

#[test]
fn non_string_secret_values_are_scrubbed_by_their_toml_text() {
    let scrubber = scrubber_for("steamgriddb_api_key = 123456789012");
    assert_eq!(
        scrub_str(&scrubber, "line: steamgriddb_api_key = 123456789012"),
        "line: steamgriddb_api_key = <redacted>"
    );
}

#[test]
fn short_values_are_not_scrubbed_from_free_text() {
    let scrubber = scrubber_for("steamgriddb_api_key = \"abc\"");
    assert_eq!(scrub_str(&scrubber, "abcdef"), "abcdef");
}

#[test]
fn settings_secrets_include_tap_url_userinfo() {
    let token = "ghp-0123456789abcdef";
    for settings in [
        format!("[[community_taps]]\nurl = \"https://bot:{token}@host/r.git\"\n"),
        // Escaped `@`: only the parsed value reveals the URL.
        format!("[[community_taps]]\nurl = \"https://bot:{token}\\u0040host/r.git\"\n"),
        // Unparseable: the raw-text scan still finds the URL.
        format!("[[community_taps]]\nurl = https://bot:{token}@host/r.git\n"),
    ] {
        let scrubber = scrubber_for(&settings);
        assert_eq!(
            scrub_str(
                &scrubber,
                &format!("clone https://bot:{token}@host/r.git; {token}")
            ),
            "clone https://<redacted>host/r.git; <redacted>",
            "{settings}"
        );
    }
}

#[test]
fn secret_env_names_match_key_as_a_word_and_other_parts_anywhere() {
    for name in [
        "API_KEY",
        "KEY_FILE",
        "STEAM_API_KEY",
        "key",
        "API_TOKEN",
        "my_password",
        "CLIENT_SECRET",
        "DB_PASSWD",
        "GITHUBTOKEN",
    ] {
        assert!(is_secret_env_name(name), "{name}");
    }
    for name in [
        "SDL_KEYBOARD_GRAB",
        "HOTKEY",
        "KEYMAP",
        "DXVK_ASYNC",
        "PROTON_LOG",
        "AUTHOR",
        "WINEDEBUG",
    ] {
        assert!(!is_secret_env_name(name), "{name}");
    }
}

fn scan(line: &str) -> Vec<String> {
    scan_secret_lines(line, |key| key == "K")
}

#[test]
fn line_scan_reads_multiline_string_openers_and_escapes() {
    assert_eq!(scan("K = \"\"\"triple secret\"\"\" x"), ["triple secret"]);
    assert_eq!(scan("K = \"\"\"unterminated"), ["unterminated"]);
    assert_eq!(scan("K = '''lit'eral''' x"), ["lit'eral"]);
    assert_eq!(
        scan("K = \"es\\\"cape\\\\d\" x"),
        ["es\\\"cape\\\\d", "es\"cape\\d"]
    );
    assert_eq!(scan("K = 'no\\escape' x"), ["no\\escape"]);
}

#[test]
fn line_scan_splits_array_elements() {
    assert_eq!(
        scan("K = [ \"first\", 'sec ond', bare , \"q\\\"x\" ] # c"),
        ["first", "sec ond", "bare", "q\\\"x", "q\"x"]
    );
    assert_eq!(scan("K = [one,two]"), ["one", "two"]);
    assert_eq!(scan("x = { K = val, other = 1 }"), ["val,", "val"]);
}

#[test]
fn redact_secret_env_vars_masks_nested_tables_and_reports_values() {
    let mut table: toml::Table = "[launch.custom_env_vars]\n\
         API_TOKEN = \"tok\"\nEMPTY_KEY = \"\"\nDXVK_ASYNC = \"1\"\n\
         [[collections]]\n[collections.custom_env_vars]\nPIN_SECRET = 1234\n\
         [other]\nAPI_TOKEN = \"not-an-env-var\"\n"
        .parse()
        .unwrap();
    let mut redacted = Vec::new();

    redact_secret_env_vars(&mut table, &mut redacted);

    let env = table["launch"]["custom_env_vars"].as_table().unwrap();
    assert_eq!(env["API_TOKEN"].as_str(), Some(REDACTED_MARKER));
    assert_eq!(env["EMPTY_KEY"].as_str(), Some(""));
    assert_eq!(env["DXVK_ASYNC"].as_str(), Some("1"));
    assert_eq!(
        table["collections"][0]["custom_env_vars"]["PIN_SECRET"].as_str(),
        Some(REDACTED_MARKER)
    );
    assert_eq!(table["other"]["API_TOKEN"].as_str(), Some("not-an-env-var"));
    redacted.sort();
    assert_eq!(redacted, ["1234", "tok"]);
}
