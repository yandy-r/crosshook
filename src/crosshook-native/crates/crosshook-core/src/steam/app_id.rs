//! Steam App ID validation shared across profile editing, saving and art lookup.

use std::fmt;

/// Maximum number of ASCII decimal digits accepted for a Steam App ID.
pub const MAX_STEAM_APP_ID_DIGITS: usize = 12;

/// Error returned when a Steam App ID string fails validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SteamAppIdValidationError {
    /// Input exceeded [`MAX_STEAM_APP_ID_DIGITS`] digits.
    TooLong { len: usize, max: usize },
    /// Input contained a character outside ASCII decimal digits (`0..=9`).
    NonAsciiDigits,
}

impl fmt::Display for SteamAppIdValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLong { len, max } => {
                write!(f, "Steam App ID must be at most {max} digits, got {len}")
            }
            Self::NonAsciiDigits => {
                write!(f, "Steam App ID must contain only numeric digits (0-9)")
            }
        }
    }
}

impl std::error::Error for SteamAppIdValidationError {}

/// Convenience alias matching `steam::app_id::ValidationError`.
pub type ValidationError = SteamAppIdValidationError;

/// Validates a Steam App ID string.
///
/// Accepts:
/// - empty string (`""`, meaning "not set")
/// - 1–12 ASCII decimal digits (`0..=9`)
///
/// Rejects:
/// - any non-ASCII-digit character (including whitespace, signs, decimals, and Unicode digits)
/// - strings longer than [`MAX_STEAM_APP_ID_DIGITS`] digits
pub fn validate_steam_app_id(value: &str) -> Result<(), ValidationError> {
    if value.is_empty() {
        return Ok(());
    }
    if !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(SteamAppIdValidationError::NonAsciiDigits);
    }
    if value.len() > MAX_STEAM_APP_ID_DIGITS {
        return Err(SteamAppIdValidationError::TooLong {
            len: value.len(),
            max: MAX_STEAM_APP_ID_DIGITS,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_empty_and_valid_ascii_digit_ids() {
        assert_eq!(validate_steam_app_id(""), Ok(()));
        assert_eq!(validate_steam_app_id("0"), Ok(()));
        assert_eq!(validate_steam_app_id("570"), Ok(()));
        assert_eq!(validate_steam_app_id("1245620"), Ok(()));
        assert_eq!(validate_steam_app_id("123456789012"), Ok(()));
    }

    #[test]
    fn rejects_more_than_twelve_digits() {
        assert_eq!(
            validate_steam_app_id("1234567890123"),
            Err(SteamAppIdValidationError::TooLong { len: 13, max: 12 })
        );
    }

    #[test]
    fn rejects_non_ascii_digits_including_whitespace_and_unicode() {
        for bad in [
            "abc",
            "123abc",
            "12.3",
            "12 3",
            " 123",
            "123 ",
            "-123",
            "+123",
            "123\n",
            "١٢٣",
            "not-a-number-at-all",
        ] {
            assert_eq!(
                validate_steam_app_id(bad),
                Err(SteamAppIdValidationError::NonAsciiDigits),
                "expected NonAsciiDigits for {bad:?}"
            );
        }
    }
}
