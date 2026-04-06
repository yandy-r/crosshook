use super::PrefixDepsError;
use std::collections::HashSet;
use std::sync::LazyLock;

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

/// Known winetricks verbs commonly used for game trainers.
/// This is advisory only -- unknown verbs that pass structural validation are allowed.
static KNOWN_VERBS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "vcrun2019",
        "vcrun2022",
        "dotnet48",
        "dotnet40",
        "dotnet35",
        "d3dx9",
        "d3dcompiler_47",
        "dxvk",
        "xact",
        "xinput",
        "corefonts",
        "allfonts",
    ]
    .into_iter()
    .collect()
});

/// Returns true if the verb is in the known-good list (advisory, not blocking).
pub fn is_known_verb(verb: &str) -> bool {
    KNOWN_VERBS.contains(verb)
}

/// Validate a batch of protontricks/winetricks verb names.
///
/// Security gates (hard reject):
/// - Empty verb list
/// - More than 50 verbs (DoS prevention)
/// - Any verb starting with `-` (flag injection -- S-06)
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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(!is_known_verb("somecustomverb123"));
    }

    #[test]
    fn known_verbs_are_recognized() {
        assert!(is_known_verb("vcrun2019"));
        assert!(is_known_verb("dotnet48"));
        assert!(is_known_verb("d3dcompiler_47"));
        assert!(!is_known_verb("unknown_verb"));
    }
}
