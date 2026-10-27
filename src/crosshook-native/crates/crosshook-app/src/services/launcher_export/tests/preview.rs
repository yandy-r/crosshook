//! `LauncherExportService` tests: preview slice.

use super::helpers::{Harness, Metadata};
use crate::AppErrorKind;

#[tokio::test]
async fn preview_script_adds_placement_header_and_writes_nothing() {
    let harness = Harness::new(Metadata::Enabled);
    let preview = harness
        .service
        .preview_script(harness.request("Prev Script"))
        .await
        .unwrap();

    let script_path = harness.script_path("prev-script");
    assert!(preview.starts_with(&format!(
        "# Save this file to: {0}\n# Make executable: chmod +x {0}\n\n",
        script_path.display()
    )));
    assert!(!harness.launchers_dir().exists());
}

#[tokio::test]
async fn preview_desktop_adds_permission_header_and_writes_nothing() {
    let harness = Harness::new(Metadata::Enabled);
    let preview = harness
        .service
        .preview_desktop(harness.request("Prev Desk"))
        .await
        .unwrap();

    let desktop_path = harness.desktop_path("prev-desk");
    assert!(preview.starts_with(&format!(
        "# Save this file to: {}\n# Permissions: 644 (not executable)\n\n",
        desktop_path.display()
    )));
    assert!(preview.contains("[Desktop Entry]"));
    assert!(!harness.applications_dir().exists());
}

#[tokio::test]
async fn previews_reject_invalid_requests() {
    let harness = Harness::new(Metadata::Disabled);
    let mut request = harness.request("Bad");
    request.trainer_path = String::new();

    let script = harness
        .service
        .preview_script(request.clone())
        .await
        .unwrap_err();
    let desktop = harness.service.preview_desktop(request).await.unwrap_err();
    for error in [script, desktop] {
        assert_eq!(error.kind(), AppErrorKind::Validation { field: "request" });
        assert_eq!(
            error.message(),
            "External launcher export requires a trainer path."
        );
    }
}
