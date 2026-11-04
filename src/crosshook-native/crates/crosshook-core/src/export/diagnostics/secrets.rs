//! Credential handling for diagnostic bundles.
//!
//! Secret settings values are redacted in `settings.toml`, and as defence in
//! depth every archived entry is scrubbed of the configured secret values, so
//! a credential echoed by a log line or a parse error never leaves the machine.

use std::fs;

use crate::settings::SettingsStore;

/// Top-level settings keys holding credentials.
pub(super) const SECRET_SETTINGS_KEYS: &[&str] = &["steamgriddb_api_key"];

/// Marker written in place of a redacted credential value.
pub(super) const REDACTED_MARKER: &str = "<redacted>";

/// Values shorter than this are not scrubbed from free text: they are too
/// likely to match unrelated bytes and are not plausible API keys.
const MIN_SCRUB_LEN: usize = 8;

/// Replaces every non-empty secret value in `settings` with [`REDACTED_MARKER`].
pub(super) fn redact_secret_settings(settings: &mut toml::Table) {
    for key in SECRET_SETTINGS_KEYS {
        if let Some(value) = settings.get_mut(*key) {
            let is_empty = value.as_str().is_some_and(str::is_empty);
            if !is_empty {
                *value = toml::Value::String(REDACTED_MARKER.to_string());
            }
        }
    }
}

/// Scrubs known secret values from archive entry contents.
#[derive(Default)]
pub(super) struct SecretScrubber {
    secrets: Vec<Vec<u8>>,
}

impl SecretScrubber {
    /// Collects the secret values configured in the settings file. Falls back
    /// to a line scan when the file does not parse, because parse errors and
    /// logs can still echo the raw value.
    pub(super) fn from_settings(settings_store: &SettingsStore) -> Self {
        let Ok(content) = fs::read_to_string(settings_store.settings_path()) else {
            return Self::default();
        };
        let mut values = match content.parse::<toml::Table>() {
            Ok(table) => SECRET_SETTINGS_KEYS
                .iter()
                .filter_map(|key| table.get(*key)?.as_str().map(str::to_string))
                .collect(),
            Err(_) => scan_secret_lines(&content),
        };
        values.retain(|value| value.len() >= MIN_SCRUB_LEN);
        values.sort_by_key(|value| std::cmp::Reverse(value.len()));
        values.dedup();
        Self {
            secrets: values.into_iter().map(String::into_bytes).collect(),
        }
    }

    /// Returns `data` with every secret occurrence replaced by [`REDACTED_MARKER`].
    pub(super) fn scrub(&self, data: &[u8]) -> Vec<u8> {
        let mut out = data.to_vec();
        for secret in &self.secrets {
            out = replace_all(&out, secret, REDACTED_MARKER.as_bytes());
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
            let value = value.trim().trim_matches(|c| c == '"' || c == '\'').trim();
            (!value.is_empty()).then(|| value.to_string())
        })
        .collect()
}

fn replace_all(haystack: &[u8], needle: &[u8], replacement: &[u8]) -> Vec<u8> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return haystack.to_vec();
    }
    let mut out = Vec::with_capacity(haystack.len());
    let mut i = 0;
    while i < haystack.len() {
        if haystack[i..].starts_with(needle) {
            out.extend_from_slice(replacement);
            i += needle.len();
        } else {
            out.push(haystack[i]);
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_secret_settings_keeps_empty_value_distinguishable() {
        let mut table: toml::Table = "steamgriddb_api_key = \"\"".parse().unwrap();
        redact_secret_settings(&mut table);
        assert_eq!(table["steamgriddb_api_key"].as_str(), Some(""));

        let mut table: toml::Table = "steamgriddb_api_key = 42".parse().unwrap();
        redact_secret_settings(&mut table);
        assert_eq!(table["steamgriddb_api_key"].as_str(), Some(REDACTED_MARKER));
    }
}
