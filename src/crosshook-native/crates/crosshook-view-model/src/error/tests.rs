use super::*;

#[test]
fn app_validation_error_keeps_field_and_message() {
    let app =
        crosshook_app::AppError::validation("profiles_directory", "directory is not writable");
    let error = ServiceError::from(&app);
    assert_eq!(
        error.kind,
        ServiceErrorKind::Validation {
            field: "profiles_directory"
        }
    );
    assert_eq!(error.message, app.message());
}

#[test]
fn app_io_error_maps_to_io() {
    let app =
        crosshook_app::AppError::from(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
    let error = ServiceError::from(app);
    assert_eq!(error.kind, ServiceErrorKind::Io);
}
