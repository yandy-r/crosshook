//! Credential handling for diagnostic bundles.
//!
//! Secret settings values are redacted in `settings.toml`, and as defence in
//! depth every archived entry is scrubbed of the configured secret values, so
//! a credential echoed by a log line or a parse error never leaves the machine.

use std::borrow::Cow;
use std::cmp::Reverse;

use memchr::memmem;

/// Top-level settings keys holding credentials (see `AppSettingsData`).
const SECRET_SETTINGS_KEYS: &[&str] = &["steamgriddb_api_key"];

/// Marker written in place of a redacted credential value.
const REDACTED_MARKER: &str = "<redacted>";

/// Values shorter than this are not scrubbed from free text: they are too
/// likely to match unrelated bytes and are not plausible API keys.
const MIN_SCRUB_LEN: usize = 8;

/// Replaces every non-empty secret value in `settings` with [`REDACTED_MARKER`].
/// Empty strings stay empty so "no key configured" remains distinguishable.
pub(super) fn redact_secret_settings(settings: &mut toml::Table) {
    for key in SECRET_SETTINGS_KEYS {
        if let Some(value) = settings.get_mut(*key) {
            if !value.as_str().is_some_and(str::is_empty) {
                *value = toml::Value::String(REDACTED_MARKER.to_string());
            }
        }
    }
}

/// Scrubs known secret values from archive entry contents.
#[derive(Default)]
pub(super) struct SecretScrubber {
    /// Longest first, so a secret containing another is replaced whole.
    finders: Vec<memmem::Finder<'static>>,
}

impl SecretScrubber {
    /// Collects the secret values configured in the raw settings file. Falls
    /// back to a line scan when the file does not parse, because parse errors
    /// and logs can still echo the raw value.
    pub(super) fn from_settings_content(content: &str) -> Self {
        let mut values: Vec<String> = match content.parse::<toml::Table>() {
            Ok(table) => SECRET_SETTINGS_KEYS
                .iter()
                .filter_map(|key| {
                    // Non-string values are scrubbed by their TOML text: typed settings
                    // loading rejects them, and that error can echo the source line.
                    table.get(*key).map(|value| match value {
                        toml::Value::String(text) => text.clone(),
                        other => other.to_string(),
                    })
                })
                .collect(),
            Err(_) => scan_secret_lines(content),
        };
        values.retain(|value| value.len() >= MIN_SCRUB_LEN);
        values.sort_by(|a, b| {
            Reverse(a.len())
                .cmp(&Reverse(b.len()))
                .then_with(|| a.cmp(b))
        });
        values.dedup();
        Self {
            finders: values
                .into_iter()
                .map(|value| memmem::Finder::new(value.as_bytes()).into_owned())
                .collect(),
        }
    }

    /// Returns `data` with every secret occurrence replaced by [`REDACTED_MARKER`];
    /// borrows when nothing matches.
    pub(super) fn scrub<'a>(&self, data: &'a [u8]) -> Cow<'a, [u8]> {
        let mut out = Cow::Borrowed(data);
        for finder in &self.finders {
            if let Some(replaced) = replace_all(&out, finder) {
                out = Cow::Owned(replaced);
            }
        }
        out
    }
}

/// Best-effort extraction of `key = value` secrets from unparseable TOML.
fn scan_secret_lines(content: &str) -> Vec<String> {
    content
        .lines()
        .filter_map(|line| {
            let (key, value) = line.split_once('=')?;
            let key = key.trim().trim_matches(|c| c == '"' || c == '\'');
            if !SECRET_SETTINGS_KEYS.contains(&key) {
                return None;
            }
            let value = extract_value(value.trim());
            (!value.is_empty()).then(|| value.to_string())
        })
        .collect()
}

/// Takes a quoted value up to its closing quote (or the line end when
/// unterminated), or an unquoted value up to whitespace or a comment.
fn extract_value(raw: &str) -> &str {
    match raw.chars().next() {
        Some(quote @ ('"' | '\'')) => {
            let rest = &raw[1..];
            rest.find(quote).map_or(rest, |end| &rest[..end])
        }
        _ => raw
            .split(|c: char| c.is_whitespace() || c == '#')
            .next()
            .unwrap_or(""),
    }
}

/// Replaces every match of `finder` in `haystack`; `None` when there is none.
fn replace_all(haystack: &[u8], finder: &memmem::Finder<'_>) -> Option<Vec<u8>> {
    let mut matches = finder.find_iter(haystack).peekable();
    matches.peek()?;
    let needle_len = finder.needle().len();
    let mut out = Vec::with_capacity(haystack.len());
    let mut last = 0;
    for start in matches {
        out.extend_from_slice(&haystack[last..start]);
        out.extend_from_slice(REDACTED_MARKER.as_bytes());
        last = start + needle_len;
    }
    out.extend_from_slice(&haystack[last..]);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "sgdb-test-key-0123456789abcdef";

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
            let scrubber = SecretScrubber::from_settings_content(&content);
            assert_eq!(
                scrub_str(&scrubber, &format!("x {KEY} y")),
                "x <redacted> y",
                "{line}"
            );
        }
    }

    #[test]
    fn scrub_replaces_adjacent_and_boundary_matches_and_borrows_without_match() {
        let scrubber =
            SecretScrubber::from_settings_content(&format!("steamgriddb_api_key = \"{KEY}\""));
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
        let scrubber = SecretScrubber::from_settings_content("steamgriddb_api_key = 123456789012");
        assert_eq!(
            scrub_str(&scrubber, "line: steamgriddb_api_key = 123456789012"),
            "line: steamgriddb_api_key = <redacted>"
        );
    }

    #[test]
    fn short_values_are_not_scrubbed_from_free_text() {
        let scrubber = SecretScrubber::from_settings_content("steamgriddb_api_key = \"abc\"");
        assert_eq!(scrub_str(&scrubber, "abcdef"), "abcdef");
    }
}
