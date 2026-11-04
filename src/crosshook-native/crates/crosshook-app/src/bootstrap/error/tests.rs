use super::*;

#[test]
fn user_message_redacts_home_paths() {
    let error = BootstrapError::StoreInit {
        store: StoreKind::Settings,
        detail: "cannot open /home/private-person/.config/crosshook".into(),
    };
    assert!(error.user_message().contains("$HOME/.config/crosshook"));
    assert!(!error.user_message().contains("private-person"));
    assert!(error.to_string().contains("private-person"));
}

#[test]
fn already_running_has_a_plain_user_message() {
    assert_eq!(
        BootstrapError::AlreadyRunning.user_message(),
        "another CrossHook instance is already running"
    );
}
