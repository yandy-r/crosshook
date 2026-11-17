use super::PrefixDepsError;

/// Maximum verbs per single install batch (DoS prevention).
const MAX_VERBS_PER_BATCH: usize = 50;

/// Structural regex: lowercase alphanumeric start, then alphanumeric/underscore/hyphen, max 64 chars.
fn is_valid_verb_structure(verb: &str) -> bool {
    if verb.is_empty() || verb.len() > 64 {
        return false;
    }
    let bytes = verb.as_bytes();
    // Must start with lowercase letter or digit
    if !bytes[0].is_ascii_lowercase() && !bytes[0].is_ascii_digit() {
        return false;
    }
    // Rest: lowercase letters, digits, underscores, hyphens
    bytes[1..]
        .iter()
        .all(|&b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

/// Validate a batch of protontricks/winetricks verb names.
///
/// Security gates (hard reject):
/// - Empty verb list
/// - More than 50 verbs (DoS prevention)
/// - Any verb starting with `-` (flag injection)
/// - Any verb failing structural regex `^[a-z0-9][a-z0-9_\-]{0,63}$`
pub fn validate_protontricks_verbs(verbs: &[String]) -> Result<(), PrefixDepsError> {
    if verbs.is_empty() {
        return Err(PrefixDepsError::ValidationError(
            "at least one verb is required".to_string(),
        ));
    }

    if verbs.len() > MAX_VERBS_PER_BATCH {
        return Err(PrefixDepsError::ValidationError(format!(
            "too many verbs ({}) — maximum is {MAX_VERBS_PER_BATCH}",
            verbs.len()
        )));
    }

    let mut invalid: Vec<String> = Vec::new();

    for verb in verbs {
        let escaped = verb.escape_default().to_string();
        if verb.starts_with('-') {
            invalid.push(format!("'{escaped}' starts with '-' (flag injection)"));
            continue;
        }
        if !is_valid_verb_structure(verb) {
            invalid.push(format!("'{escaped}' fails structural validation"));
        }
    }

    if !invalid.is_empty() {
        return Err(PrefixDepsError::ValidationError(format!(
            "invalid verb(s): {}",
            invalid.join("; ")
        )));
    }

    Ok(())
}

/// Parse a Steam App ID into its canonical decimal form.
///
/// Accepts surrounding whitespace and ASCII digits only; the value must fit a
/// `u32` and be non-zero (`" 0012 "` → `"12"`). Anything else is `None`.
pub fn parse_nonzero_steam_app_id(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if !trimmed.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let parsed = trimmed.parse::<u32>().ok()?;
    if parsed == 0 {
        return None;
    }
    Some(parsed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_nonzero_steam_app_id_rejects_invalid_ids() {
        for raw in [
            "",
            "  ",
            "0",
            "00",
            "abc",
            "-1",
            "+5",
            "--help",
            "4294967296",
        ] {
            assert_eq!(parse_nonzero_steam_app_id(raw), None, "accepted {raw:?}");
        }
    }

    #[test]
    fn parse_nonzero_steam_app_id_returns_canonical_decimal() {
        assert_eq!(
            parse_nonzero_steam_app_id(" 12345 ").as_deref(),
            Some("12345")
        );
        assert_eq!(parse_nonzero_steam_app_id("007").as_deref(), Some("7"));
        assert_eq!(
            parse_nonzero_steam_app_id("4294967295").as_deref(),
            Some("4294967295")
        );
    }

    #[test]
    fn valid_verbs_pass() {
        let verbs = vec!["vcrun2019".to_string(), "dotnet48".to_string()];
        assert!(validate_protontricks_verbs(&verbs).is_ok());
    }

    #[test]
    fn reject_flag_injection() {
        let result = validate_protontricks_verbs(&["-q".to_string()]);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("flag injection"), "error: {err}");

        let result2 = validate_protontricks_verbs(&["--help".to_string()]);
        assert!(result2.is_err());
    }

    #[test]
    fn reject_shell_metachar() {
        let result = validate_protontricks_verbs(&["vcrun;rm -rf".to_string()]);
        assert!(result.is_err());

        let result2 = validate_protontricks_verbs(&["dotnet$(cmd)".to_string()]);
        assert!(result2.is_err());
    }

    #[test]
    fn reject_empty_list() {
        let result = validate_protontricks_verbs(&[]);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("at least one verb"), "error: {err}");
    }

    #[test]
    fn reject_too_many_verbs() {
        let verbs: Vec<String> = (0..51).map(|i| format!("verb{i}")).collect();
        let result = validate_protontricks_verbs(&verbs);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("too many"), "error: {err}");
    }

    #[test]
    fn reject_empty_verb() {
        let result = validate_protontricks_verbs(&["".to_string()]);
        assert!(result.is_err());
    }

    #[test]
    fn unknown_verb_passes_structural_validation() {
        let verbs = vec!["somecustomverb123".to_string()];
        assert!(validate_protontricks_verbs(&verbs).is_ok());
    }
}
