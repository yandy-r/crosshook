/// Remove ANSI escape sequences from runner output.
fn strip_ansi_codes(raw: &str) -> String {
    let mut result = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for c2 in chars.by_ref() {
                if c2.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Strip absolute filesystem paths from stderr before user display.
///
/// Replaces path-looking tokens (starting with `/home/`, `/tmp/`, `/var/`) with `<path>`.
/// This is a simple character-scan approach -- no regex dependency required.
pub(super) fn sanitize_stderr(raw: &str) -> String {
    let raw = strip_ansi_codes(raw);
    let prefixes: &[&str] = &["/home/", "/tmp/", "/var/"];
    let mut result = String::with_capacity(raw.len());
    let mut i = 0;
    let bytes = raw.as_bytes();
    while i < bytes.len() {
        let remaining = &raw[i..];
        let mut matched = false;
        for &prefix in prefixes {
            if remaining.starts_with(prefix) {
                // Consume the path token up to the next whitespace or colon.
                let end = remaining
                    .find(|c: char| c.is_whitespace() || c == ':')
                    .unwrap_or(remaining.len());
                result.push_str("<path>");
                i += end;
                matched = true;
                break;
            }
        }
        if !matched {
            // Advance one character safely.
            let ch_len = raw[i..].chars().next().map(char::len_utf8).unwrap_or(1);
            result.push_str(&raw[i..i + ch_len]);
            i += ch_len;
        }
    }
    // Truncate to prevent huge error messages (character-safe).
    if result.chars().count() > 500 {
        let truncated: String = result.chars().take(500).collect();
        format!("{truncated}...(truncated)")
    } else {
        result
    }
}

/// Sanitize a single runner output line for UI display.
pub fn sanitize_output_for_ui(raw: &str) -> String {
    sanitize_stderr(raw)
}
