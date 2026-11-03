//! Install root authority: `target_root` must canonically equal a
//! backend-resolved install root; the installer receives the trusted spelling.
//! The install root listing returns exactly that trusted set.
//!
//! The default root source trusts `<tmp>/Steam`, deriving
//! `<tmp>/Steam/compatibilitytools.d`. Every case is checked on both
//! `install_version` and `start_install`.

use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use crosshook_core::protonup::{ProtonUpInstallErrorKind, ProtonUpInstallRequest};

use super::super::super::messages::{MSG_INSTALL_ROOT_UNTRUSTED, MSG_NO_WRITABLE_INSTALL_ROOT};
use super::super::super::progress::next_progress;
use super::super::helpers::{
    finished, ge_catalog, native_root, owner, request, FakeRoots, Harness, Mode,
};
use crate::AppErrorKind;

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn targeted(target: &Path, force: bool) -> ProtonUpInstallRequest {
    ProtonUpInstallRequest {
        target_root: text(target),
        force,
        ..request("GE-Proton9-21")
    }
}

/// `<tmp>/Steam/compatibilitytools.d`, created.
fn trusted_root(h: &Harness) -> PathBuf {
    let root = h.steam_root().join("compatibilitytools.d");
    fs::create_dir_all(&root).unwrap();
    root
}

/// `<tmp>/Other/compatibilitytools.d`, created; not derived from a trusted root.
fn foreign_root(h: &Harness) -> PathBuf {
    let root = h.root.path().join("Other").join("compatibilitytools.d");
    fs::create_dir_all(&root).unwrap();
    root
}

/// Asserts both entry points refuse `request` as untrusted before anything
/// runs.
async fn assert_refused(h: &Harness, request: ProtonUpInstallRequest) {
    assert_refused_with(h, request, MSG_INSTALL_ROOT_UNTRUSTED).await;
}

/// Asserts both entry points refuse `request` with `text` before anything
/// runs.
async fn assert_refused_with(h: &Harness, request: ProtonUpInstallRequest, text: &str) {
    let result = h.service.install_version(request.clone()).await.unwrap();
    assert!(!result.success, "legacy install accepted {request:?}");
    assert_eq!(
        result.error_kind,
        Some(ProtonUpInstallErrorKind::InvalidPath)
    );
    assert_eq!(result.error_message.as_deref(), Some(text));
    h.assert_rejected();

    let error = match h
        .service
        .start_install(
            owner("main"),
            request.clone(),
            ge_catalog().versions.remove(0),
        )
        .await
    {
        Ok(_) => panic!("start_install accepted {request:?}"),
        Err(error) => error,
    };
    assert_eq!(
        error.kind(),
        AppErrorKind::Validation {
            field: "target_root"
        }
    );
    assert_eq!(error.message(), text);
    h.assert_rejected();
}

/// Runs `request` through both entry points to completion.
async fn install_both(h: &Harness, request: ProtonUpInstallRequest) {
    let result = h.service.install_version(request.clone()).await.unwrap();
    assert!(result.success, "legacy install refused: {result:?}");
    let mut started = match h
        .service
        .start_install(owner("main"), request, ge_catalog().versions.remove(0))
        .await
    {
        Ok(started) => started,
        Err(error) => panic!("start_install refused: {}", error.message()),
    };
    while next_progress(&mut started.progress).await.is_some() {}
    finished(&h.jobs).await;
}

#[tokio::test]
async fn foreign_roots_are_refused_by_both_entry_points() {
    let h = Harness::new(Mode::Succeed);
    let trusted = trusted_root(&h);
    let foreign = foreign_root(&h);
    let traversal = trusted.join("..").join("compatibilitytools.d");

    for target in [
        text(&foreign),
        text(&traversal),
        "compatibilitytools.d".to_string(),
        text(&foreign.join("GE-Proton9-21")),
    ] {
        let request = ProtonUpInstallRequest {
            target_root: target,
            ..request("GE-Proton9-21")
        };
        assert_refused(&h, request).await;
    }
    assert!(h.installer.target_roots().is_empty());
}

#[tokio::test]
async fn force_against_a_foreign_root_removes_nothing() {
    let h = Harness::new(Mode::Succeed);
    let foreign = foreign_root(&h);
    let existing = foreign.join("GE-Proton9-21");
    fs::create_dir_all(&existing).unwrap();
    let sentinel = existing.join("sentinel");
    fs::write(&sentinel, b"keep").unwrap();

    assert_refused(&h, targeted(&foreign, true)).await;

    assert!(
        sentinel.exists(),
        "forced install removed a foreign directory"
    );
}

#[tokio::test]
async fn symlinked_alias_of_a_trusted_root_installs_into_the_trusted_spelling() {
    let h = Harness::new(Mode::Succeed);
    let trusted = trusted_root(&h);
    let steam_alias = h.root.path().join("SteamAlias");
    symlink(h.steam_root(), &steam_alias).unwrap();
    let root_alias = h.root.path().join("compat-alias");
    symlink(&trusted, &root_alias).unwrap();

    for target in [steam_alias.join("compatibilitytools.d"), root_alias] {
        install_both(&h, targeted(&target, false)).await;
    }

    let seen = h.installer.target_roots();
    assert_eq!(seen.len(), 4);
    assert!(
        seen.iter().all(|root| *root == text(&trusted)),
        "installer received an untrusted spelling: {seen:?}"
    );
}

#[tokio::test]
async fn empty_target_installs_into_the_backend_default_root() {
    let h = Harness::new(Mode::Succeed);
    let trusted = h.steam_root().join("compatibilitytools.d");

    install_both(&h, request("GE-Proton9-21")).await;

    assert_eq!(h.installer.target_roots(), vec![text(&trusted); 2]);
}

#[tokio::test]
async fn missing_trusted_root_is_accepted_with_the_trusted_spelling() {
    let h = Harness::new(Mode::Succeed);
    let trusted = h.steam_root().join("compatibilitytools.d");
    assert!(!trusted.exists());

    install_both(&h, targeted(&trusted, false)).await;

    assert_eq!(h.installer.target_roots(), vec![text(&trusted); 2]);
}

#[tokio::test]
async fn home_relative_target_matches_a_home_trusted_root() {
    let home = crosshook_core::platform::base_dirs()
        .expect("home directory")
        .home_dir()
        .to_path_buf();
    let home_root = home.join(".local/share/Steam/compatibilitytools.d");
    let fixed = home_root.clone();
    let h = Harness::with_roots(Mode::Succeed, move |root| {
        FakeRoots::new(vec![root.join("Steam")], vec![native_root(fixed)])
    });

    let tilde = ProtonUpInstallRequest {
        target_root: "~/.local/share/Steam/compatibilitytools.d".to_string(),
        ..request("GE-Proton9-21")
    };
    install_both(&h, tilde).await;
    assert_eq!(h.installer.target_roots(), vec![text(&home_root); 2]);
}

#[tokio::test]
async fn other_user_home_target_is_refused() {
    let h = Harness::new(Mode::Succeed);
    let request = ProtonUpInstallRequest {
        target_root: "~otheruser/.local/share/Steam/compatibilitytools.d".to_string(),
        ..request("GE-Proton9-21")
    };

    assert_refused(&h, request).await;
}

#[tokio::test]
async fn nested_root_under_a_trusted_root_is_refused() {
    let h = Harness::new(Mode::Succeed);
    let nested = trusted_root(&h).join("sub").join("compatibilitytools.d");
    fs::create_dir_all(&nested).unwrap();

    assert_refused(&h, targeted(&nested, false)).await;
}

#[tokio::test]
async fn empty_target_without_a_writable_trusted_root_is_refused() {
    let h = Harness::with_roots(Mode::Succeed, |root| {
        let mut read_only = native_root(root.join("ReadOnly").join("compatibilitytools.d"));
        read_only.writable = false;
        read_only.reason = Some("path-read-only".to_string());
        FakeRoots::new(Vec::new(), vec![read_only])
    });

    assert_refused_with(&h, request("GE-Proton9-21"), MSG_NO_WRITABLE_INSTALL_ROOT).await;
}

#[tokio::test]
async fn listing_matches_the_roots_install_trusts() {
    let h = Harness::new(Mode::Succeed);
    let trusted = trusted_root(&h);
    let foreign_steam = h.root.path().join("Other");
    let foreign = foreign_root(&h);

    let listed = h
        .service
        .resolve_install_roots(Some(text(&foreign_steam)))
        .await
        .unwrap();

    let paths: Vec<PathBuf> = listed.iter().map(|root| root.path.clone()).collect();
    assert_eq!(paths, vec![trusted.clone()], "untrusted Steam path listed");
    assert!(listed[0].writable);

    assert_refused(&h, targeted(&foreign, false)).await;
    install_both(&h, targeted(&listed[0].path, false)).await;
    assert_eq!(h.installer.target_roots(), vec![text(&trusted); 2]);
}
