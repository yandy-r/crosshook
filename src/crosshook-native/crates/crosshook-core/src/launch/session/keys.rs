//! Session-registry profile-key derivation.
//!
//! Shared by launch registration (game/trainer commands) and the
//! user-initiated "reset" command so both derive the same registry key for
//! a given profile name — a mismatch here would make reset silently miss
//! (or wrongly hit) sessions.

/// Session registry key for launches that lack a user-facing profile name.
/// Unlikely in practice — profile-name validation reserves this exact value
/// (see `crate::profile::legacy::validate_name` and
/// `crate::profile::toml_store::utils::validate_name`), so a real saved
/// profile can never collide with it — but keeps the registry lookup robust
/// if an un-validated request still slips a blank/anonymous name through.
pub const ANONYMOUS_PROFILE_KEY: &str = "__crosshook_anonymous_profile__";

/// Derives the session registry key for a profile name. Both launch
/// registration and the user-initiated reset command derive keys through
/// this helper so registrations and lookups always match.
pub fn session_profile_key_for_name(profile_name: Option<&str>) -> String {
    profile_name
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(ANONYMOUS_PROFILE_KEY)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_profile_key_trims_profile_names() {
        assert_eq!(
            session_profile_key_for_name(Some("  Synthetic Quest  ")),
            "Synthetic Quest"
        );
    }

    #[test]
    fn session_profile_key_falls_back_to_anonymous_for_missing_or_blank_names() {
        let anonymous = session_profile_key_for_name(None);
        assert!(!anonymous.is_empty());
        assert_eq!(session_profile_key_for_name(Some("   ")), anonymous);
        assert_eq!(session_profile_key_for_name(Some("")), anonymous);
    }

    #[test]
    fn anonymous_profile_key_is_the_fallback_value() {
        assert_eq!(session_profile_key_for_name(None), ANONYMOUS_PROFILE_KEY);
    }
}
