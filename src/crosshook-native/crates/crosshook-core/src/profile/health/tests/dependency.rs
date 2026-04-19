use super::super::{build_dependency_health_issues, HealthIssueSeverity};
use super::fixtures::make_dep_row;

#[test]
fn installed_verbs_produce_no_issues() {
    let deps = vec![
        make_dep_row("vcrun2019", "installed"),
        make_dep_row("dotnet48", "installed"),
    ];
    let required = vec!["vcrun2019".to_string(), "dotnet48".to_string()];
    let issues = build_dependency_health_issues(&deps, &required, "/tmp/pfx");
    assert!(issues.is_empty());
}

#[test]
fn missing_verb_produces_warning() {
    let deps = vec![make_dep_row("vcrun2019", "missing")];
    let required = vec!["vcrun2019".to_string()];
    let issues = build_dependency_health_issues(&deps, &required, "/tmp/pfx");
    assert_eq!(issues.len(), 1);
    assert!(matches!(issues[0].severity, HealthIssueSeverity::Warning));
    assert!(issues[0].message.contains("not installed"));
}

#[test]
fn unknown_state_produces_warning() {
    let deps = vec![];
    let required = vec!["vcrun2019".to_string()];
    let issues = build_dependency_health_issues(&deps, &required, "/tmp/pfx");
    // Empty dep_states with required verbs → "not been checked" warning
    assert_eq!(issues.len(), 1);
    assert!(issues[0].message.contains("not been checked"));
}

#[test]
fn skipped_verb_produces_info() {
    let deps = vec![make_dep_row("vcrun2019", "user_skipped")];
    let required = vec!["vcrun2019".to_string()];
    let issues = build_dependency_health_issues(&deps, &required, "/tmp/pfx");
    assert_eq!(issues.len(), 1);
    assert!(matches!(issues[0].severity, HealthIssueSeverity::Info));
    assert!(issues[0].message.contains("skipped"));
}

#[test]
fn empty_required_verbs_produce_no_issues() {
    let deps = vec![make_dep_row("vcrun2019", "installed")];
    let required: Vec<String> = vec![];
    let issues = build_dependency_health_issues(&deps, &required, "/tmp/pfx");
    assert!(issues.is_empty());
}
