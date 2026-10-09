//! Presenter helpers: user-facing message sanitization (home-path redaction).

use std::path::Path;

/// Placeholder substituted for the redacted home directory.
pub(crate) const HOME_PLACEHOLDER: &str = "$HOME";

/// Redacts the `HOME` environment value in `message` at path-component
/// boundaries, then always scrubs remaining generic `/home/<user>` and
/// `/var/home/<user>` components — other users' home paths stay private too.
pub(crate) fn sanitize_message(message: &str) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    sanitize_message_with_home(message, &home)
}

/// Deterministic core of [`sanitize_message`]: explicit `home` redaction
/// followed by the generic `/home/<user>` and `/var/home/<user>` scrub.
pub(crate) fn sanitize_message_with_home(message: &str, home: &str) -> String {
    scrub_generic_home(&sanitize_with_home(message, home))
}

/// Renders `path` for display with the `HOME` environment directory redacted.
///
/// Reads `HOME` once; an unset or empty value falls back to generic
/// `/home/<user>` scrubbing. Non-UTF-8 components are shown lossily. Lexical:
/// the path is treated as already resolved; no canonicalization, symlink
/// resolution, or filesystem access. See [`sanitize_display_path_with_home`]
/// for the deterministic variant.
pub fn sanitize_display_path(path: &Path) -> String {
    let home = std::env::var_os("HOME");
    sanitize_display_path_with_home(path, home.as_deref().map(Path::new))
}

/// Renders `path` for display with `home` redacted to `$HOME`.
///
/// Only whole path components match: a neighbour such as `/home/user2` (or
/// `/home/userñ`) is left intact when `home` is `/home/user`. A `/home/<user>`
/// home also covers its canonical Fedora alias `/var/home/<user>`. `None` or
/// an empty `home` falls back to generic `/home/<user>` scrubbing.
/// Deterministic: no environment access. Lexical: no canonicalization, symlink
/// resolution, or filesystem access.
pub fn sanitize_display_path_with_home(path: &Path, home: Option<&Path>) -> String {
    let text = path.to_string_lossy();
    match home {
        Some(home) if !home.as_os_str().is_empty() => {
            let home = home.to_string_lossy();
            let mut redacted = sanitize_with_home(&text, &home);
            if let Some(alias) = var_home_alias(&home) {
                redacted = sanitize_with_home(&redacted, &alias);
            }
            redacted
        }
        _ => scrub_generic_home(&text),
    }
}

/// Fedora-style `/var/home/<user>` spelling of a `/home/<user>` home, if any.
fn var_home_alias(home: &str) -> Option<String> {
    home.trim_end_matches(['/', '\\'])
        .strip_prefix("/home/")
        .filter(|user| !user.is_empty())
        .map(|user| format!("/var/home/{user}"))
}

/// Redacts `home` in `message` where it appears as a whole path component
/// sequence (including mid-string, e.g. after `path=` or a quote). A longer
/// directory that merely starts with `home` (neighbour such as
/// `/home/user2`) is left intact. Deterministic: no environment access.
pub(crate) fn sanitize_with_home(message: &str, home: &str) -> String {
    let home = home.trim_end_matches(['/', '\\']);
    if home.len() < 2 || home == "/" {
        return message.to_string();
    }
    redact_at_boundaries(message, home)
}

fn redact_at_boundaries(message: &str, needle: &str) -> String {
    let mut out = String::with_capacity(message.len());
    let mut cursor = 0;
    while let Some(offset) = message[cursor..].find(needle) {
        let start = cursor + offset;
        let end = start + needle.len();
        let before_ok = !message[..start]
            .chars()
            .next_back()
            .is_some_and(is_name_char);
        // Trailing dots before a non-name char are sentence punctuation.
        let after_ok = !message[end..]
            .trim_start_matches('.')
            .chars()
            .next()
            .is_some_and(is_name_char);
        if before_ok && after_ok {
            out.push_str(&message[cursor..start]);
            out.push_str(HOME_PLACEHOLDER);
            cursor = end;
        } else {
            out.push_str(&message[cursor..end]);
            cursor = end;
        }
    }
    out.push_str(&message[cursor..]);
    out
}

/// Characters that can extend a path component; a match adjacent to one is a
/// partial component (e.g. neighbour home suffix) and must not be redacted.
fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | '.')
}

/// Generic fallback scrubbing `/home/<user>` and its canonical Fedora alias
/// `/var/home/<user>`: rewrites them to `$HOME`, keeping the suffix.
fn scrub_generic_home(message: &str) -> String {
    const HOME_PREFIX: &str = "/home/";
    const VAR_SEGMENT: &str = "/var";
    let mut out = String::with_capacity(message.len());
    let mut cursor = 0;
    while let Some(offset) = message[cursor..].find(HOME_PREFIX) {
        let home_at = cursor + offset;
        // `/var/home/…` is the `/home/…` alias; widen the match window.
        let start = if home_at >= VAR_SEGMENT.len()
            && message.get(home_at - VAR_SEGMENT.len()..home_at) == Some(VAR_SEGMENT)
        {
            home_at - VAR_SEGMENT.len()
        } else {
            home_at
        };
        if message[..start]
            .chars()
            .next_back()
            .is_some_and(is_name_char)
        {
            // Mid-word occurrence such as `x/home/…`; not a home path.
            out.push_str(&message[cursor..home_at + HOME_PREFIX.len()]);
            cursor = home_at + HOME_PREFIX.len();
            continue;
        }
        let name_start = home_at + HOME_PREFIX.len();
        let name_len = message[name_start..]
            .find(|c: char| !is_name_char(c))
            .unwrap_or(message.len() - name_start);
        // Trailing dots are sentence punctuation, not part of the component.
        let name_end = name_start
            + message[name_start..name_start + name_len]
                .trim_end_matches('.')
                .len();
        if name_end == name_start {
            // `/home/` with no user component follows; leave alone.
            out.push_str(&message[cursor..name_start]);
            cursor = name_start;
            continue;
        }
        out.push_str(&message[cursor..start]);
        out.push_str(HOME_PLACEHOLDER);
        cursor = name_end;
    }
    out.push_str(&message[cursor..]);
    out
}

#[cfg(test)]
mod tests;
