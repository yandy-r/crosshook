use crate::launch::request::{LaunchValidationIssue, ValidationError, ValidationSeverity};

#[test]
fn validation_error_help_explains_missing_steam_compatdata_path() {
    assert_eq!(
        ValidationError::SteamCompatDataPathMissing.help(),
        "Launch the game through Steam at least once to create the compatibility data directory."
    );
}

#[test]
fn validation_error_help_explains_missing_launch_optimization_dependency() {
    assert_eq!(
        ValidationError::LaunchOptimizationDependencyMissing {
            option_id: "use_gamemode".to_string(),
            dependency: "gamemoderun".to_string(),
        }
        .help(),
        "Install 'gamemoderun' and make sure it is available on PATH, or disable 'use_gamemode'."
    );
}

#[test]
fn validation_error_severity_is_fatal_for_current_variants() {
    assert_eq!(
        ValidationError::NativeWindowsExecutableNotSupported.severity(),
        ValidationSeverity::Fatal
    );
    assert_eq!(
        ValidationError::UnsupportedMethod("direct".to_string()).severity(),
        ValidationSeverity::Fatal
    );
}

#[test]
fn validation_error_issue_packages_message_help_and_severity() {
    assert_eq!(
        ValidationError::UnsupportedMethod("direct".to_string()).issue(),
        LaunchValidationIssue {
            message:
                "Unsupported launch method 'direct'. Use steam_applaunch, proton_run, or native."
                    .to_string(),
            help:
                "Change the profile launch method to 'steam_applaunch', 'proton_run', or 'native'."
                    .to_string(),
            severity: ValidationSeverity::Fatal,
            code: Some("unsupported_method".to_string()),
            trainer_hash_stored: None,
            trainer_hash_current: None,
            trainer_sha256_community: None,
        }
    );
}

#[test]
fn unshare_net_unavailable_is_warning_severity() {
    let err = ValidationError::UnshareNetUnavailable;
    assert_eq!(err.severity(), ValidationSeverity::Warning);
    let issue = err.issue();
    assert_eq!(issue.severity, ValidationSeverity::Warning);
    assert!(issue.message.contains("unshare"));
    assert!(issue.help.contains("Kernel policy"));
    assert!(issue.message.contains("--net"));
}

#[test]
fn offline_readiness_insufficient_is_warning_severity() {
    let err = ValidationError::OfflineReadinessInsufficient {
        score: 40,
        reasons: vec!["missing hash".to_string()],
    };
    assert_eq!(err.severity(), ValidationSeverity::Warning);
    let issue = err.issue();
    assert_eq!(issue.severity, ValidationSeverity::Warning);
    assert!(issue.message.contains("40"));
}

#[test]
fn low_disk_space_advisory_is_warning_severity() {
    let err = ValidationError::LowDiskSpaceAdvisory {
        available_mb: 512,
        threshold_mb: 2048,
        mount_path: "/home/test".to_string(),
    };
    assert_eq!(err.severity(), ValidationSeverity::Warning);
    let issue = err.issue();
    assert_eq!(issue.severity, ValidationSeverity::Warning);
    assert!(issue.message.contains("512"));
    assert!(issue.help.contains("/home/test"));
}

#[test]
fn validation_error_codes_are_populated() {
    assert_eq!(
        ValidationError::GamePathRequired.code(),
        "game_path_required"
    );
    assert_eq!(
        ValidationError::SteamAppIdRequired.code(),
        "steam_app_id_required"
    );
    assert_eq!(
        ValidationError::RuntimePrefixPathRequired.code(),
        "runtime_prefix_path_required"
    );
    assert_eq!(
        ValidationError::RuntimeProtonPathRequired.code(),
        "runtime_proton_path_required"
    );
    assert_eq!(
        ValidationError::UnknownLaunchOptimization("foo".into()).code(),
        "unknown_launch_optimization"
    );
    assert_eq!(
        ValidationError::UnsupportedMethod("x".into()).code(),
        "unsupported_method"
    );
    assert_eq!(
        ValidationError::OfflineReadinessInsufficient {
            score: 0,
            reasons: vec![],
        }
        .code(),
        "offline_readiness_insufficient"
    );
    assert_eq!(
        ValidationError::LowDiskSpaceAdvisory {
            available_mb: 0,
            threshold_mb: 1,
            mount_path: "/".into(),
        }
        .code(),
        "low_disk_space_advisory"
    );

    let issue = ValidationError::GamePathRequired.issue();
    assert_eq!(issue.code.as_deref(), Some("game_path_required"));
}
