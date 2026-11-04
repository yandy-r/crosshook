//! Credential handling for diagnostic bundles.
//!
//! Secret settings values and secret-named profile environment variables are
//! redacted, and as defence in depth every archived entry is scrubbed of the
//! configured secret values, so a credential echoed by a log line or a parse
//! error never leaves the machine.

use std::borrow::Cow;
use std::cmp::Reverse;

use memchr::memmem;

use super::url_credentials::{
    structured_userinfos, userinfo_secrets, userinfo_slugs, userinfos_in,
};

/// Top-level settings keys holding credentials (see `AppSettingsData`).
const SECRET_SETTINGS_KEYS: &[&str] = &["steamgriddb_api_key"];

/// Profile tables whose entries are environment variables.
const ENV_VAR_TABLE: &str = "custom_env_vars";

/// Environment variable names containing any of these (ASCII
/// case-insensitive) are treated as credentials. `PASSWD` is the common
/// abbreviation of `PASSWORD`; broader parts such as `AUTH` are left out
/// because they also match ordinary names (`AUTHOR`).
const SECRET_ENV_NAME_PARTS: &[&str] = &["KEY", "TOKEN", "SECRET", "PASSWORD", "PASSWD"];

/// Marker written in place of a redacted credential value.
pub(super) const REDACTED_MARKER: &str = "<redacted>";

/// Values shorter than this are not scrubbed from free text: they are too
/// likely to match unrelated bytes and are not plausible API keys.
const MIN_SCRUB_LEN: usize = 8;

/// Replaces every non-empty secret value in `settings` with [`REDACTED_MARKER`].
/// Empty strings stay empty so "no key configured" remains distinguishable.
pub(super) fn redact_secret_settings(settings: &mut toml::Table) {
    for key in SECRET_SETTINGS_KEYS {
        if let Some(value) = settings.get_mut(*key) {
            redact_value(value);
        }
    }
}

/// Returns `true` when an environment variable named `name` holds a credential.
pub(super) fn is_secret_env_name(name: &str) -> bool {
    let name = name.to_ascii_uppercase();
    SECRET_ENV_NAME_PARTS.iter().any(|part| name.contains(part))
}

/// Redacts secret-named entries of every `custom_env_vars` table at any depth
/// of `table`, appending the original scalar values to `redacted`. A
/// `custom_env_vars` value that is not a table cannot be read entry by entry,
/// so it is redacted whole.
pub(super) fn redact_secret_env_vars(table: &mut toml::Table, redacted: &mut Vec<String>) {
    for (key, value) in table.iter_mut() {
        if key == ENV_VAR_TABLE {
            match value.as_table_mut() {
                Some(env) => {
                    for (name, value) in env.iter_mut() {
                        if is_secret_env_name(name) {
                            redacted.extend(secret_scalars(value));
                            redact_value(value);
                        }
                    }
                }
                None => {
                    redacted.extend(secret_scalars(value));
                    redact_value(value);
                    continue;
                }
            }
        }
        match value {
            toml::Value::Table(inner) => redact_secret_env_vars(inner, redacted),
            toml::Value::Array(items) => {
                for item in items.iter_mut().filter_map(toml::Value::as_table_mut) {
                    redact_secret_env_vars(item, redacted);
                }
            }
            _ => {}
        }
    }
}

/// Replaces `value` with [`REDACTED_MARKER`] unless it is an empty string.
fn redact_value(value: &mut toml::Value) {
    if !value.as_str().is_some_and(str::is_empty) {
        *value = toml::Value::String(REDACTED_MARKER.to_string());
    }
}

/// Non-empty texts a secret value appears as, one per scalar: strings as
/// themselves, other scalars as their TOML text, arrays and tables element by
/// element. Typed loading rejects non-string values, and its error quotes
/// the source line, where each scalar appears verbatim.
fn secret_scalars(value: &toml::Value) -> Vec<String> {
    let mut scalars = Vec::new();
    push_scalars(value, &mut scalars);
    scalars
}

fn push_scalars(value: &toml::Value, scalars: &mut Vec<String>) {
    match value {
        toml::Value::String(text) if text.is_empty() => {}
        toml::Value::String(text) => scalars.push(text.clone()),
        toml::Value::Array(items) => items.iter().for_each(|item| push_scalars(item, scalars)),
        toml::Value::Table(table) => table.values().for_each(|item| push_scalars(item, scalars)),
        other => scalars.push(other.to_string()),
    }
}

/// Secret values configured in the raw settings file: credential settings and
/// the userinfo of every URL (community tap URLs included). Falls back to a
/// line scan for credential settings when the file does not parse, because
/// parse errors and logs can still echo the raw value.
pub(super) fn settings_secrets(content: &str) -> Vec<String> {
    let (mut values, tap_userinfos) = match content.parse::<toml::Table>() {
        Ok(table) => (
            SECRET_SETTINGS_KEYS
                .iter()
                .filter_map(|key| table.get(*key))
                .flat_map(secret_scalars)
                .collect(),
            tap_url_userinfos(&table),
        ),
        Err(_) => (
            scan_secret_lines(content, |key| SECRET_SETTINGS_KEYS.contains(&key)),
            Vec::new(),
        ),
    };
    let userinfos = userinfos_in(content)
        .into_iter()
        .chain(tap_userinfos.iter().map(String::as_str));
    for userinfo in userinfos {
        values.extend(userinfo_secrets(userinfo));
        // Tap folder names are slugs of the tap URL and appear in log paths.
        values.extend(userinfo_slugs(userinfo));
    }
    values
}

/// Userinfo of the parsed `community_taps[*].url` values (covers escaped
/// spellings and quote characters the raw-text scan cannot see).
fn tap_url_userinfos(settings: &toml::Table) -> Vec<String> {
    settings
        .get("community_taps")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|tap| tap.get("url")?.as_str())
        .flat_map(structured_userinfos)
        .map(str::to_string)
        .collect()
}

/// Scrubs known secret values from archive entry contents.
#[derive(Default)]
pub(super) struct SecretScrubber {
    /// Longest first, so a secret containing another is replaced whole.
    finders: Vec<memmem::Finder<'static>>,
}

impl SecretScrubber {
    /// Builds a free-text scrubber for `values`, ignoring those shorter than
    /// [`MIN_SCRUB_LEN`].
    pub(super) fn new(values: impl IntoIterator<Item = String>) -> Self {
        Self::with_exact(values, std::iter::empty())
    }

    /// Like [`SecretScrubber::new`], but also replaces every non-empty `exact`
    /// value whatever its length. For text known to quote parsed field
    /// values, such as profile load errors: there a short credential is
    /// still a credential, and over-redacting a diagnostic message only
    /// costs readability.
    pub(super) fn with_exact(
        values: impl IntoIterator<Item = String>,
        exact: impl IntoIterator<Item = String>,
    ) -> Self {
        let mut values: Vec<String> = values
            .into_iter()
            .filter(|value| value.len() >= MIN_SCRUB_LEN)
            .chain(exact.into_iter().filter(|value| !value.is_empty()))
            .collect();
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

    /// Scrubs `text` in place.
    pub(super) fn scrub_string(&self, text: &mut String) {
        if let Cow::Owned(scrubbed) = self.scrub(text.as_bytes()) {
            // Needles and haystack are UTF-8, so every match lies on char
            // boundaries and the result stays UTF-8.
            *text = String::from_utf8_lossy(&scrubbed).into_owned();
        }
    }
}

/// Best-effort extraction of `key = value` values whose key satisfies
/// `is_secret_key` from unparseable TOML, including the pairs of inline
/// tables (`name = { KEY = value, ... }`).
pub(super) fn scan_secret_lines(
    content: &str,
    is_secret_key: impl Fn(&str) -> bool,
) -> Vec<String> {
    let mut values = Vec::new();
    for line in content.lines() {
        let pairs = std::iter::once(line).chain(
            line.match_indices(['{', ','])
                .map(|(index, _)| &line[index + 1..]),
        );
        for pair in pairs {
            let Some((key, value)) = pair.split_once('=') else {
                continue;
            };
            let key = key.trim().trim_matches(|c| c == '"' || c == '\'');
            if is_secret_key(key) {
                values.extend(extract_values(value.trim()));
            }
        }
    }
    values
}

/// Takes a quoted value up to its closing quote (or the line end when
/// unterminated), or an unquoted value up to whitespace or a comment, and
/// also its prefix before an inline-table `,` or `}`.
fn extract_values(raw: &str) -> Vec<String> {
    let values = match raw.chars().next() {
        Some(quote @ ('"' | '\'')) => {
            let rest = &raw[1..];
            vec![rest.find(quote).map_or(rest, |end| &rest[..end])]
        }
        _ => {
            let value = raw
                .split(|c: char| c.is_whitespace() || c == '#')
                .next()
                .unwrap_or("");
            let inline = value.split([',', '}']).next().unwrap_or("");
            if inline == value {
                vec![value]
            } else {
                vec![value, inline]
            }
        }
    };
    values
        .into_iter()
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
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
#[path = "secrets_tests.rs"]
mod tests;
