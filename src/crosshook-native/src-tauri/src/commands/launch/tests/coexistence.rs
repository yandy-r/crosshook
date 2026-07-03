use crosshook_core::launch::{
    LaunchValidationIssue, ValidationSeverity, MAX_COEXISTENCE_ADVISORY_RECORDS,
};

use crate::commands::launch::warnings::coexistence_records_from_warnings;

fn issue(code: Option<&str>, severity: ValidationSeverity) -> LaunchValidationIssue {
    LaunchValidationIssue {
        message: "test issue".to_string(),
        help: "test help".to_string(),
        severity,
        code: code.map(str::to_string),
        trainer_hash_stored: None,
        trainer_hash_current: None,
        trainer_sha256_community: None,
        hook_id: None,
        hook_name: None,
        hook_stage: None,
        hook_exit_code: None,
        hook_timed_out: None,
    }
}

#[test]
fn coexistence_records_filter_prefix_and_cap_at_16() {
    let mut warnings = vec![
        issue(Some("trainer_hash_mismatch"), ValidationSeverity::Warning),
        issue(None, ValidationSeverity::Info),
    ];
    for index in 0..(MAX_COEXISTENCE_ADVISORY_RECORDS + 4) {
        warnings.push(issue(
            Some(&format!("mod_coexistence_rule_{index}")),
            if index % 2 == 0 {
                ValidationSeverity::Warning
            } else {
                ValidationSeverity::Info
            },
        ));
    }

    let records = coexistence_records_from_warnings(&warnings);

    assert_eq!(records.len(), MAX_COEXISTENCE_ADVISORY_RECORDS);
    assert_eq!(MAX_COEXISTENCE_ADVISORY_RECORDS, 16);
    assert!(records
        .iter()
        .all(|record| record.code.starts_with("mod_coexistence_")));
    assert_eq!(records[0].code, "mod_coexistence_rule_0");
    assert_eq!(records[0].severity, ValidationSeverity::Warning);
    assert_eq!(records[1].severity, ValidationSeverity::Info);
}
