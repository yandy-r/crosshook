use std::collections::BTreeMap;

use crate::launch::request::{validate, validate_all, ValidationError};

use super::support::{proton_request, steam_request};

#[test]
fn proton_run_validates_with_custom_env_vars() {
    let (_temp_dir, mut request) = proton_request();
    request
        .custom_env_vars
        .insert("DXVK_ASYNC".to_string(), "1".to_string());
    assert_eq!(validate(&request), Ok(()));
    assert!(validate_all(&request).is_empty());
}

#[test]
fn validate_rejects_reserved_custom_env_key() {
    let (_temp_dir, mut request) = proton_request();
    request
        .custom_env_vars
        .insert("WINEPREFIX".to_string(), "/tmp/evil".to_string());
    assert_eq!(
        validate(&request),
        Err(ValidationError::CustomEnvVarReservedKey(
            "WINEPREFIX".to_string()
        ))
    );
}

#[test]
fn validate_rejects_custom_env_key_with_equals() {
    let (_temp_dir, mut request) = proton_request();
    request
        .custom_env_vars
        .insert("A=B".to_string(), "1".to_string());
    assert_eq!(
        validate(&request),
        Err(ValidationError::CustomEnvVarKeyContainsEquals)
    );
}

#[test]
fn validate_rejects_whitespace_only_custom_env_key() {
    let (_temp_dir, mut request) = proton_request();
    request
        .custom_env_vars
        .insert("   ".to_string(), "1".to_string());
    assert_eq!(
        validate(&request),
        Err(ValidationError::CustomEnvVarKeyEmpty)
    );
}

#[test]
fn validate_rejects_nul_in_custom_env_key_and_value() {
    let (_temp_dir, mut request) = proton_request();
    request
        .custom_env_vars
        .insert("A\0B".to_string(), "1".to_string());
    assert_eq!(
        validate(&request),
        Err(ValidationError::CustomEnvVarKeyContainsNul)
    );

    let (_temp_dir, mut request) = proton_request();
    request
        .custom_env_vars
        .insert("FOO".to_string(), "bar\0baz".to_string());
    assert_eq!(
        validate(&request),
        Err(ValidationError::CustomEnvVarValueContainsNul)
    );
}

#[test]
fn validate_all_collects_multiple_custom_env_issues() {
    let (_temp_dir, mut request) = steam_request();
    request.custom_env_vars = BTreeMap::from([
        ("WINEPREFIX".to_string(), "1".to_string()),
        ("BAD=KEY".to_string(), "1".to_string()),
    ]);
    let issues = validate_all(&request);
    assert_eq!(issues.len(), 2);
    assert!(issues
        .iter()
        .any(|issue| { issue.code.as_deref() == Some("custom_env_var_reserved_key") }));
    assert!(issues
        .iter()
        .any(|issue| { issue.code.as_deref() == Some("custom_env_var_key_contains_equals") }));
}
