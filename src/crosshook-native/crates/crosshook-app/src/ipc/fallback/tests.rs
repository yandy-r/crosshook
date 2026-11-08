use super::*;

#[test]
fn unavailable_becomes_the_fallback() {
    let unavailable = AppError::new(AppErrorKind::Unavailable, "no art");
    assert_eq!(unavailable_or(Err(unavailable), None::<String>), Ok(None));
}

#[test]
fn success_and_other_errors_pass_through() {
    assert_eq!(unavailable_or(Ok(vec![1]), Vec::new()), Ok(vec![1]));
    let invalid = AppError::validation("app_id", "bad id");
    assert_eq!(
        unavailable_or(Err(invalid.clone()), Vec::<u8>::new()),
        Err(invalid)
    );
}

#[test]
fn cancel_maps_ok_not_found_and_other_errors() {
    assert_eq!(cancel_accepted(Ok(())), Ok(true));
    assert_eq!(
        cancel_accepted(Err(AppError::new(AppErrorKind::NotFound, "gone"))),
        Ok(false)
    );
    // The service reports an unknown op id through the workspace-path error.
    assert_eq!(
        cancel_accepted(Err(AppError::from(
            crosshook_core::fs_util::WorkspacePathError::NotFound
        ))),
        Ok(false)
    );
    let invalid = AppError::validation("version", "bad input");
    assert_eq!(cancel_accepted(Err(invalid.clone())), Err(invalid));
}
