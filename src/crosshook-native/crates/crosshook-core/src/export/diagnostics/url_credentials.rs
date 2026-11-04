//! Credentials embedded in URLs (`scheme://user:password@host/...`).

use std::borrow::Cow;

use memchr::memmem;

use crate::community::taps::slugify;

const SCHEME_SEPARATOR: &str = "://";

/// Characters ending a URL authority (RFC 3986). Used for structured values,
/// where the whole string is a value and quotes or spaces may belong to a
/// password.
fn ends_structured_authority(character: char) -> bool {
    matches!(character, '/' | '?' | '#')
}

/// Characters ending a URL authority, or the URL itself, in free text.
fn ends_free_text_authority(character: char) -> bool {
    ends_structured_authority(character)
        || matches!(character, '"' | '\'' | '<' | '>' | '`')
        || character.is_whitespace()
}

/// `(start, at)` byte offsets of the userinfo of every `scheme://userinfo@`
/// URL in `text`: the userinfo is `text[start..at]` and `text[at]` is its
/// `@`. The last `@` of an authority wins, so an unencoded `@` in a password
/// stays inside the userinfo.
fn userinfo_spans(
    text: &str,
    ends_authority: fn(char) -> bool,
) -> impl Iterator<Item = (usize, usize)> + '_ {
    memmem::find_iter(text.as_bytes(), SCHEME_SEPARATOR).filter_map(move |position| {
        let start = position + SCHEME_SEPARATOR.len();
        let after_scheme = &text[start..];
        let authority = after_scheme
            .find(ends_authority)
            .map_or(after_scheme, |end| &after_scheme[..end]);
        authority.rfind('@').map(|at| (start, start + at))
    })
}

/// Removes the userinfo of every URL in the structured value `value`:
/// `scheme://user:pass@host/repo` becomes `scheme://host/repo`. Scheme-less
/// URLs such as scp-style `git@host:org/repo.git` are returned unchanged:
/// they cannot carry a password and their user names a remote account, not
/// a credential.
pub(super) fn strip_url_userinfo(value: &str) -> Cow<'_, str> {
    let mut stripped: Option<String> = None;
    let mut last = 0;
    for (start, at) in userinfo_spans(value, ends_structured_authority) {
        let out = stripped.get_or_insert_with(|| String::with_capacity(value.len()));
        out.push_str(&value[last..start]);
        last = at + 1;
    }
    match stripped {
        Some(mut out) => {
            out.push_str(&value[last..]);
            Cow::Owned(out)
        }
        None => Cow::Borrowed(value),
    }
}

/// Non-empty userinfo strings (`user[:password]`) of every URL in free text.
pub(super) fn userinfos_in(text: &str) -> Vec<&str> {
    userinfos(text, ends_free_text_authority)
}

/// Non-empty userinfo strings of every URL in the structured value `value`.
pub(super) fn structured_userinfos(value: &str) -> Vec<&str> {
    userinfos(value, ends_structured_authority)
}

fn userinfos(text: &str, ends_authority: fn(char) -> bool) -> Vec<&str> {
    userinfo_spans(text, ends_authority)
        .filter(|(start, at)| at > start)
        .map(|(start, at)| &text[start..at])
        .collect()
}

/// The password of `userinfo`, or the whole userinfo when there is none
/// (e.g. a token used as the user name).
fn credential(userinfo: &str) -> &str {
    userinfo
        .split_once(':')
        .map_or(userinfo, |(_, password)| password)
}

/// Strings to scrub for one userinfo: `userinfo@` as a whole and its
/// credential, each also in percent-decoded form when that differs.
pub(super) fn userinfo_secrets(userinfo: &str) -> Vec<String> {
    let mut secrets = vec![format!("{userinfo}@"), credential(userinfo).to_string()];
    if let Some(decoded) = percent_decode(userinfo) {
        secrets.push(format!("{decoded}@"));
        secrets.push(credential(&decoded).to_string());
    }
    secrets
}

/// Slug spellings of one userinfo, as they appear in community tap folder
/// names (`slugify` of the tap URL).
pub(super) fn userinfo_slugs(userinfo: &str) -> [String; 2] {
    [slugify(userinfo), slugify(credential(userinfo))]
}

/// Decodes `%XX` escapes; `None` when there are none or the result is not
/// UTF-8. Malformed escapes are kept verbatim.
fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    if !bytes.contains(&b'%') {
        return None;
    }
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let escaped = (bytes[index] == b'%')
            .then(|| bytes.get(index + 1..index + 3))
            .flatten()
            .and_then(|hex| u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok());
        match escaped {
            Some(byte) => {
                decoded.push(byte);
                index += 3;
            }
            None => {
                decoded.push(bytes[index]);
                index += 1;
            }
        }
    }
    String::from_utf8(decoded)
        .ok()
        .filter(|decoded| decoded != text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_url_userinfo_removes_user_and_password() {
        for (url, expected) in [
            (
                "https://user:tok3n@github.com/org/repo.git",
                "https://github.com/org/repo.git",
            ),
            (
                "https://tok3n@github.com/org/repo",
                "https://github.com/org/repo",
            ),
            ("https://u:p@ss@host:8443/r?x=1", "https://host:8443/r?x=1"),
            ("ssh://git@host/org/repo.git", "ssh://host/org/repo.git"),
            ("https://host", "https://host"),
            // Quotes and spaces are password characters in a structured value.
            ("https://u:pa'ss \"x@host/r", "https://host/r"),
            (
                "http://a:b@proxy:3128 http://c:d@proxy2",
                "http://proxy:3128 http://proxy2",
            ),
        ] {
            assert_eq!(strip_url_userinfo(url), expected, "{url}");
        }
    }

    #[test]
    fn strip_url_userinfo_keeps_urls_without_userinfo() {
        for url in [
            "https://github.com/org/repo.git",
            "https://host/path/@user/repo",
            "git@github.com:org/repo.git",
            "/local/tap/path",
            "",
        ] {
            assert!(matches!(strip_url_userinfo(url), Cow::Borrowed(_)), "{url}");
        }
    }

    #[test]
    fn userinfos_in_finds_every_url_credential_in_free_text() {
        let text = "fetch 'https://a:b@h1/r' then \"http://tok@h2\" and https://h3/@x\n\
                    ssh://git@h4:22/r";
        assert_eq!(userinfos_in(text), ["a:b", "tok", "git"]);
    }

    #[test]
    fn structured_userinfos_keep_quotes_in_passwords() {
        assert_eq!(structured_userinfos("https://u:pa'ss@host/r"), ["u:pa'ss"]);
        assert!(userinfos_in("https://u:pa'ss@host/r").is_empty());
    }

    #[test]
    fn userinfo_secrets_cover_whole_userinfo_credential_and_decoded_forms() {
        assert_eq!(userinfo_secrets("user:pass"), ["user:pass@", "pass"]);
        assert_eq!(userinfo_secrets("token"), ["token@", "token"]);
        assert_eq!(
            userinfo_secrets("u:p%40ss%zz"),
            ["u:p%40ss%zz@", "p%40ss%zz", "u:p@ss%zz@", "p@ss%zz"]
        );
    }

    #[test]
    fn userinfo_slugs_match_tap_folder_spelling() {
        assert_eq!(
            userinfo_slugs("reader:ghp_Tok"),
            ["reader-ghp-tok", "ghp-tok"]
        );
    }
}
