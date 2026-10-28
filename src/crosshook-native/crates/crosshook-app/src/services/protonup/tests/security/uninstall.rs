//! AC5: uninstall root authority and containment, against real temp dirs.
//!
//! The root source trusts `<tmp>/Steam` only. Every refusal is asserted for
//! both `plan_uninstall` and `uninstall`, with a fixed path-free text and the
//! sentinels outside the install root untouched.

use std::fs;
use std::path::{Path, PathBuf};

use super::super::helpers::{Harness, Mode};
use crate::services::protonup::messages::{UNINSTALL_NOT_FOUND, UNINSTALL_OUTSIDE};

fn harness() -> Harness {
    Harness::new(Mode::Succeed)
}

fn compat_root(h: &Harness) -> PathBuf {
    let root = h.steam_root().join("compatibilitytools.d");
    fs::create_dir_all(&root).unwrap();
    root
}

/// A directory outside the install root holding a sentinel file.
fn outside_dir(h: &Harness, name: &str) -> (PathBuf, PathBuf) {
    let dir = h.root.path().join(name);
    fs::create_dir_all(&dir).unwrap();
    let sentinel = dir.join("sentinel");
    fs::write(&sentinel, b"keep").unwrap();
    (dir, sentinel)
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn assert_path_free(h: &Harness, message: &str) {
    assert!(!message.contains('/'), "message contains '/': {message}");
    assert!(
        !message.contains(h.root.path().to_string_lossy().as_ref()),
        "message contains the tmp path: {message}"
    );
}

/// Asserts that plan and uninstall both refuse `path` with `expected`.
async fn assert_refused(
    h: &Harness,
    path: &str,
    steam_client_install_path: Option<String>,
    expected: &str,
) {
    let plan = h
        .service
        .plan_uninstall(path.to_string(), steam_client_install_path.clone())
        .await
        .unwrap();
    assert!(!plan.success, "plan accepted {path}");
    assert!(plan.conflicting_app_ids.is_empty());
    let plan_message = plan.error_message.expect("plan refusal carries a message");
    assert_eq!(plan_message, expected);
    assert_path_free(h, &plan_message);

    let response = h
        .service
        .uninstall(path.to_string(), steam_client_install_path)
        .await
        .unwrap();
    assert!(!response.success, "uninstall accepted {path}");
    assert!(response.conflicting_app_ids.is_empty());
    let message = response
        .error_message
        .expect("uninstall refusal carries a message");
    assert_eq!(message, expected);
    assert_path_free(h, &message);
}

#[tokio::test]
async fn parent_dir_ending_outside_the_root_is_refused() {
    let h = harness();
    let root = compat_root(&h);
    let (outside, sentinel) = outside_dir(&h, "outside-target");
    let traversal = format!(
        "{}/../../{}",
        text(&root),
        outside.file_name().unwrap().to_string_lossy()
    );

    assert_refused(&h, &traversal, None, UNINSTALL_OUTSIDE).await;

    assert!(outside.exists());
    assert!(sentinel.exists());
}

#[tokio::test]
async fn parent_dir_resolving_back_inside_the_root_is_refused() {
    let h = harness();
    let root = compat_root(&h);
    let tool = root.join("GE-Proton-Inside-Fixture");
    fs::create_dir_all(&tool).unwrap();
    fs::write(tool.join("proton"), b"x").unwrap();
    let traversal = format!(
        "{}/GE-Proton-Inside-Fixture/../GE-Proton-Inside-Fixture",
        text(&root)
    );

    assert_refused(&h, &traversal, None, UNINSTALL_OUTSIDE).await;

    assert!(tool.join("proton").exists());
}

#[cfg(unix)]
#[tokio::test]
async fn final_symlink_escaping_the_root_is_refused() {
    let h = harness();
    let root = compat_root(&h);
    let (outside, sentinel) = outside_dir(&h, "outside-final");
    let link = root.join("GE-Proton-Link-Fixture");
    std::os::unix::fs::symlink(&outside, &link).unwrap();

    assert_refused(&h, &text(&link), None, UNINSTALL_OUTSIDE).await;

    assert!(outside.exists());
    assert!(sentinel.exists());
    assert!(
        link.symlink_metadata().is_ok(),
        "the link itself is untouched"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn intermediate_symlink_escaping_the_root_is_refused() {
    let h = harness();
    let root = compat_root(&h);
    let (outside, sentinel) = outside_dir(&h, "outside-intermediate");
    let victim = outside.join("GE-Proton-Victim-Fixture");
    fs::create_dir_all(&victim).unwrap();
    fs::write(victim.join("proton"), b"x").unwrap();
    let link = root.join("escape");
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    let through_link = link.join("GE-Proton-Victim-Fixture");

    assert_refused(&h, &text(&through_link), None, UNINSTALL_OUTSIDE).await;

    assert!(victim.join("proton").exists());
    assert!(sentinel.exists());
}

#[tokio::test]
async fn sibling_directory_sharing_the_root_prefix_is_refused() {
    let h = harness();
    compat_root(&h);
    let sibling_root = h.steam_root().join("compatibilitytools.d-evil");
    let tool = sibling_root.join("GE-Proton-Sibling-Fixture");
    fs::create_dir_all(&tool).unwrap();
    fs::write(tool.join("proton"), b"x").unwrap();

    assert_refused(&h, &text(&tool), None, UNINSTALL_OUTSIDE).await;

    assert!(tool.join("proton").exists());
}

#[tokio::test]
async fn the_install_root_itself_is_refused() {
    let h = harness();
    let root = compat_root(&h);
    let tool = root.join("GE-Proton-Survivor-Fixture");
    fs::create_dir_all(&tool).unwrap();

    assert_refused(&h, &text(&root), None, UNINSTALL_OUTSIDE).await;

    assert!(root.exists());
    assert!(tool.exists());
}

#[tokio::test]
async fn nested_child_of_a_tool_is_refused() {
    let h = harness();
    let root = compat_root(&h);
    let tool = root.join("GE-Proton-Parent-Fixture");
    let nested = tool.join("files").join("bin");
    fs::create_dir_all(&nested).unwrap();

    assert_refused(&h, &text(&nested), None, UNINSTALL_OUTSIDE).await;

    assert!(nested.exists());
}

#[tokio::test]
async fn unrelated_directory_outside_every_root_is_refused() {
    let h = harness();
    compat_root(&h);
    let (outside, sentinel) = outside_dir(&h, "outside-plain");

    assert_refused(&h, &text(&outside), None, UNINSTALL_OUTSIDE).await;

    assert!(sentinel.exists());
}

#[tokio::test]
async fn relative_path_is_not_resolved_against_the_process_directory() {
    let h = harness();
    compat_root(&h);

    assert_refused(&h, "compatibilitytools.d", None, UNINSTALL_OUTSIDE).await;
}

#[tokio::test]
async fn relative_name_of_an_existing_tool_is_refused_and_not_deleted() {
    let h = harness();
    compat_root(&h);
    // `src` exists relative to the test process directory (the crate root).
    let existing = Path::new("src");
    assert!(existing.is_dir(), "fixture assumption: ./src exists");

    assert_refused(&h, "src", None, UNINSTALL_OUTSIDE).await;
    assert_refused(&h, "./src", None, UNINSTALL_OUTSIDE).await;

    assert!(existing.is_dir(), "relative target must not be deleted");
}

#[cfg(unix)]
#[tokio::test]
async fn tool_reached_through_a_steam_root_alias_is_planned_and_removed() {
    let h = harness();
    let root = compat_root(&h);
    let tool = root.join("GE-Proton-Alias-Fixture");
    fs::create_dir_all(&tool).unwrap();
    fs::write(tool.join("proton"), b"x").unwrap();
    let alias = h.root.path().join("SteamAlias");
    std::os::unix::fs::symlink(h.steam_root(), &alias).unwrap();
    let via_alias = alias
        .join("compatibilitytools.d")
        .join("GE-Proton-Alias-Fixture");

    let plan = h
        .service
        .plan_uninstall(text(&via_alias), None)
        .await
        .unwrap();
    assert!(plan.success, "{plan:?}");
    assert!(tool.exists());

    let response = h.service.uninstall(text(&via_alias), None).await.unwrap();
    assert!(response.success, "{response:?}");
    assert!(!tool.exists());
    assert!(root.exists());
}

#[cfg(unix)]
#[tokio::test]
async fn alias_with_a_missing_parent_is_not_found() {
    let h = harness();
    compat_root(&h);
    let ghost = h
        .root
        .path()
        .join("NoSuchAlias")
        .join("compatibilitytools.d")
        .join("GE-Proton-Ghost-Fixture");

    assert_refused(&h, &text(&ghost), None, UNINSTALL_NOT_FOUND).await;
}

#[tokio::test]
async fn caller_supplied_fake_steam_root_is_ignored() {
    let h = harness();
    compat_root(&h);
    let fake_steam = h.root.path().join("FakeSteam");
    fs::create_dir_all(fake_steam.join("steamapps")).unwrap();
    let tool = fake_steam
        .join("compatibilitytools.d")
        .join("GE-Proton-Fake-Fixture");
    fs::create_dir_all(&tool).unwrap();
    fs::write(tool.join("proton"), b"x").unwrap();

    assert_refused(&h, &text(&tool), Some(text(&fake_steam)), UNINSTALL_OUTSIDE).await;

    assert!(tool.join("proton").exists(), "fake-root tool must survive");
}

#[tokio::test]
async fn caller_supplied_nonexistent_steam_root_is_ignored() {
    let h = harness();
    let root = compat_root(&h);
    let tool = root.join("GE-Proton-Trusted-Fixture");
    fs::create_dir_all(&tool).unwrap();
    let ghost = h.root.path().join("does-not-exist");

    let response = h
        .service
        .uninstall(text(&tool), Some(text(&ghost)))
        .await
        .unwrap();

    assert!(response.success, "trusted root still applies: {response:?}");
    assert!(!tool.exists());
}

#[tokio::test]
async fn caller_path_equal_to_the_trusted_root_is_honoured() {
    let h = harness();
    let root = compat_root(&h);
    let tool = root.join("GE-Proton-Match-Fixture");
    fs::create_dir_all(&tool).unwrap();

    let response = h
        .service
        .uninstall(text(&tool), Some(text(&h.steam_root())))
        .await
        .unwrap();

    assert!(response.success, "{response:?}");
    assert!(!tool.exists());
}

#[tokio::test]
async fn valid_direct_child_plans_then_removes_only_that_directory() {
    let h = harness();
    let root = compat_root(&h);
    let tool = root.join("GE-Proton-Target-Fixture");
    let sibling = root.join("GE-Proton-Sibling-Fixture");
    for dir in [&tool, &sibling] {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("proton"), b"x").unwrap();
    }
    let (outside, sentinel) = outside_dir(&h, "outside-sentinel");

    let plan = h.service.plan_uninstall(text(&tool), None).await.unwrap();
    assert!(plan.success, "{plan:?}");
    assert!(tool.exists());

    let response = h.service.uninstall(text(&tool), None).await.unwrap();
    assert!(response.success, "{response:?}");
    assert_eq!(response.error_message, None);
    assert!(!tool.exists());
    assert!(sibling.join("proton").exists());
    assert!(root.exists());
    assert!(outside.exists());
    assert!(sentinel.exists());
}

#[tokio::test]
async fn proton_file_input_normalizes_to_its_tool_directory() {
    let h = harness();
    let proton = h.make_tool("GE-Proton-File-Fixture");
    let tool = proton.parent().unwrap().to_path_buf();

    let plan = h.service.plan_uninstall(text(&proton), None).await.unwrap();
    assert!(plan.success, "{plan:?}");

    let response = h.service.uninstall(text(&proton), None).await.unwrap();
    assert!(response.success, "{response:?}");
    assert!(!tool.exists());
}

#[tokio::test]
async fn invalid_path_roots_never_authorize_a_target() {
    use crosshook_core::protonup::install_root::{InstallRootCandidate, InstallRootKind};

    use super::super::helpers::FakeRoots;

    let h = Harness::with_roots(Mode::Succeed, |root| {
        let steam = root.join("Steam");
        let ct = steam.join("compatibilitytools.d");
        fs::create_dir_all(&ct).unwrap();
        FakeRoots::new(
            vec![steam],
            vec![InstallRootCandidate {
                kind: InstallRootKind::NativeSteam,
                path: ct,
                writable: false,
                reason: Some("invalid-path".to_string()),
            }],
        )
    });
    let tool = compat_root(&h).join("GE-Proton-Invalid-Fixture");
    fs::create_dir_all(&tool).unwrap();

    assert_refused(&h, &text(&tool), None, UNINSTALL_OUTSIDE).await;

    assert!(tool.exists());
}
