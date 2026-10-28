//! Behaviour of `plan_uninstall` / `uninstall` on a valid install root.

use std::fs;
use std::path::PathBuf;

use super::super::messages::{UNINSTALL_NOT_FOUND, UNINSTALL_OUTSIDE};
use super::helpers::{Harness, Mode};

fn harness() -> Harness {
    Harness::new(Mode::Succeed)
}

/// `<root>/Steam/compatibilitytools.d/<name>` with a `proton` file inside.
fn tool_dir(harness: &Harness, name: &str) -> PathBuf {
    harness
        .make_tool(name)
        .parent()
        .expect("proton file has a parent")
        .to_path_buf()
}

fn write_mapping(harness: &Harness, app_id: &str, tool: &str) {
    let config = harness.steam_root().join("config");
    fs::create_dir_all(&config).unwrap();
    fs::write(
        config.join("config.vdf"),
        format!(r#""root" {{ "CompatToolMapping" {{ "{app_id}" {{ "name" "{tool}" }} }} }}"#),
    )
    .unwrap();
}

fn path_string(path: &std::path::Path) -> String {
    path.to_string_lossy().into_owned()
}

#[tokio::test]
async fn plan_surfaces_conflicting_app_ids_from_the_steam_config() {
    let h = harness();
    let tool = tool_dir(&h, "GE-Proton-Mapped-Fixture");
    write_mapping(&h, "424242", "GE-Proton-Mapped-Fixture");

    let plan = h
        .service
        .plan_uninstall(path_string(&tool), None)
        .await
        .unwrap();

    assert!(plan.success);
    assert_eq!(plan.error_message, None);
    assert!(plan.conflicting_app_ids.contains(&"424242".to_string()));
    assert!(tool.exists(), "planning must not delete anything");
}

#[tokio::test]
async fn plan_without_a_mapping_reports_no_conflicts() {
    let h = harness();
    let tool = tool_dir(&h, "GE-Proton-Unmapped-Fixture");

    let plan = h
        .service
        .plan_uninstall(path_string(&tool), None)
        .await
        .unwrap();

    assert!(plan.success);
    assert!(plan.conflicting_app_ids.is_empty());
    assert!(tool.exists());
}

#[tokio::test]
async fn uninstall_removes_only_the_requested_tool() {
    let h = harness();
    let tool = tool_dir(&h, "GE-Proton-Remove-Fixture");
    let sibling = tool_dir(&h, "GE-Proton-Keep-Fixture");

    let response = h.service.uninstall(path_string(&tool), None).await.unwrap();

    assert!(response.success);
    assert_eq!(response.error_message, None);
    assert!(response.conflicting_app_ids.is_empty());
    assert!(!tool.exists());
    assert!(sibling.join("proton").exists());
}

#[tokio::test]
async fn uninstall_replans_and_never_trusts_an_earlier_plan() {
    let h = harness();
    let tool = tool_dir(&h, "GE-Proton-Replan-Fixture");

    let plan = h
        .service
        .plan_uninstall(path_string(&tool), None)
        .await
        .unwrap();
    assert!(plan.success);

    fs::remove_dir_all(&tool).unwrap();
    let response = h.service.uninstall(path_string(&tool), None).await.unwrap();

    assert!(!response.success);
    assert_eq!(response.error_message.as_deref(), Some(UNINSTALL_NOT_FOUND));
}

#[tokio::test]
async fn missing_tool_in_a_valid_root_uses_the_fixed_not_found_text() {
    let h = harness();
    tool_dir(&h, "GE-Proton-Present-Fixture");
    let missing = h
        .steam_root()
        .join("compatibilitytools.d")
        .join("GE-Proton-Missing-Fixture");

    let plan = h
        .service
        .plan_uninstall(path_string(&missing), None)
        .await
        .unwrap();
    let response = h
        .service
        .uninstall(path_string(&missing), None)
        .await
        .unwrap();

    assert!(!plan.success);
    assert_eq!(plan.error_message.as_deref(), Some(UNINSTALL_NOT_FOUND));
    assert!(plan.conflicting_app_ids.is_empty());
    assert!(!response.success);
    assert_eq!(response.error_message.as_deref(), Some(UNINSTALL_NOT_FOUND));
}

#[tokio::test]
async fn empty_and_blank_paths_are_refused_with_the_fixed_text() {
    let h = harness();
    for raw in ["", "   "] {
        let plan = h
            .service
            .plan_uninstall(raw.to_string(), None)
            .await
            .unwrap();
        let response = h.service.uninstall(raw.to_string(), None).await.unwrap();
        assert!(!plan.success);
        assert_eq!(plan.error_message.as_deref(), Some(UNINSTALL_OUTSIDE));
        assert!(!response.success);
        assert_eq!(response.error_message.as_deref(), Some(UNINSTALL_OUTSIDE));
    }
}

#[tokio::test]
async fn refusal_wire_shape_is_unchanged() {
    let h = harness();
    let response = h.service.uninstall(String::new(), None).await.unwrap();
    let json = serde_json::to_value(&response).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "success": false,
            "conflicting_app_ids": [],
            "error_message": UNINSTALL_OUTSIDE,
        })
    );
}
