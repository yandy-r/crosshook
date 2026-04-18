use super::super::{SteamExternalLauncherExportError, SteamExternalLauncherExportValidationError};

#[test]
fn invalid_request_display_preserves_validation_context() {
    let error = SteamExternalLauncherExportError::InvalidRequest(
        SteamExternalLauncherExportValidationError::UnsupportedMethod("native".to_string()),
    );

    assert_eq!(
        error.to_string(),
        "External launcher export only supports steam_applaunch and proton_run, not 'native'."
    );
}
