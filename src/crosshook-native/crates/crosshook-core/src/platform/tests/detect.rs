use std::ffi::{CStr, CString};
use std::fs;
use std::path::Path;

use tempfile::tempdir;

use super::super::detect::{
    document_portal_tail, is_flatpak_with, nested_document_portal_host_path,
    normalize_flatpak_host_path, parse_document_portal_tail, read_document_portal_host_path_xattr,
    read_document_portal_host_path_xattr_bytes, DOCUMENT_PORTAL_HOST_PATH_XATTR,
};
use super::common::{ScopedEnv, TEST_ENV_KEY};

#[test]
fn returns_true_when_env_var_set_and_file_absent() {
    let _guard = ScopedEnv::set(TEST_ENV_KEY, "dev.crosshook.CrossHook");
    let tmp = tempdir().unwrap();
    let missing = tmp.path().join("does-not-exist");
    assert!(is_flatpak_with(TEST_ENV_KEY, &missing));
}

#[test]
fn returns_true_when_file_present_and_env_var_unset() {
    let _guard = ScopedEnv::unset(TEST_ENV_KEY);
    let tmp = tempdir().unwrap();
    let present = tmp.path().join(".flatpak-info");
    fs::write(&present, b"[Application]\nname=test\n").unwrap();
    assert!(is_flatpak_with(TEST_ENV_KEY, &present));
}

#[test]
fn returns_true_when_both_present() {
    let _guard = ScopedEnv::set(TEST_ENV_KEY, "dev.crosshook.CrossHook");
    let tmp = tempdir().unwrap();
    let present = tmp.path().join(".flatpak-info");
    fs::write(&present, b"[Application]\nname=test\n").unwrap();
    assert!(is_flatpak_with(TEST_ENV_KEY, &present));
}

#[test]
fn returns_false_when_neither_present() {
    let _guard = ScopedEnv::unset(TEST_ENV_KEY);
    let tmp = tempdir().unwrap();
    let missing = tmp.path().join("does-not-exist");
    assert!(!is_flatpak_with(TEST_ENV_KEY, &missing));
}

#[test]
fn normalize_flatpak_host_path_strips_host_mount_prefix() {
    assert_eq!(
        normalize_flatpak_host_path("/run/host/usr/share/steam/compatibilitytools.d/proton/proton"),
        "/usr/share/steam/compatibilitytools.d/proton/proton"
    );
    assert_eq!(
        normalize_flatpak_host_path("/run/host/home/alice/Games/test.exe"),
        "/home/alice/Games/test.exe"
    );
}

#[test]
fn normalize_flatpak_host_path_leaves_non_host_paths_unchanged() {
    assert_eq!(
        normalize_flatpak_host_path(r"C:\Games\Test Game\game.exe"),
        r"C:\Games\Test Game\game.exe"
    );
    assert_eq!(
        normalize_flatpak_host_path("relative/path/to/file"),
        "relative/path/to/file"
    );
    assert_eq!(
        normalize_flatpak_host_path("  /tmp/test-path  "),
        "/tmp/test-path"
    );
}

#[test]
fn document_portal_tail_recognises_both_mount_forms() {
    assert_eq!(
        document_portal_tail("/run/user/1000/doc/abc/game.exe"),
        Some("/abc/game.exe")
    );
    assert_eq!(
        document_portal_tail("/run/flatpak/doc/abc/game.exe"),
        Some("/abc/game.exe")
    );
    assert_eq!(document_portal_tail("/run/flatpak/doc"), Some(""));
    for not_portal in [
        "/run/user/me/doc/abc/game.exe",
        "/run/user//doc/abc/game.exe",
        "/run/user/1000/docs/abc/game.exe",
        "/run/flatpak/docs/abc/game.exe",
        "/home/user/run/flatpak/doc/abc/game.exe",
    ] {
        assert_eq!(document_portal_tail(not_portal), None, "for `{not_portal}`");
    }
}

#[test]
fn parse_document_portal_tail_splits_id_and_remainder() {
    let parsed = parse_document_portal_tail("/abc/Games//sub/./game.exe").unwrap();
    assert_eq!(parsed.doc_id, "abc");
    assert_eq!(parsed.rest, vec!["sub", "game.exe"]);

    for malformed in ["", "/", "/abc", "/abc/", "//game.exe", "/abc/Games/../x"] {
        assert!(
            parse_document_portal_tail(malformed).is_none(),
            "for `{malformed}`"
        );
    }
}

/// Without a readable xattr the best-effort repair leaves portal paths in
/// either mount form unchanged (aside from trimming).
#[test]
fn normalize_flatpak_host_path_keeps_unreadable_portal_paths() {
    assert_eq!(
        normalize_flatpak_host_path(" /run/flatpak/doc/crosshook-test-missing/game.exe "),
        "/run/flatpak/doc/crosshook-test-missing/game.exe"
    );
    assert_eq!(
        normalize_flatpak_host_path("/run/user/4000000000/doc/crosshook-test-missing/game.exe"),
        "/run/user/4000000000/doc/crosshook-test-missing/game.exe"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn normalize_flatpak_host_path_resolves_document_portal_host_path_xattr() {
    let temp_dir = tempdir().unwrap();
    let portal_file = temp_dir.path().join("proton");
    std::fs::write(&portal_file, b"test").unwrap();

    let target_host_path = "/usr/share/steam/compatibilitytools.d/proton-cachyos-slr/proton";
    let c_path = CString::new(portal_file.to_string_lossy().as_bytes()).unwrap();
    let attr_name = CStr::from_bytes_with_nul(DOCUMENT_PORTAL_HOST_PATH_XATTR).unwrap();
    let attr_value = CString::new(target_host_path).unwrap();

    // SAFETY: all pointers are valid NUL-terminated strings for the duration
    // of the call; the path names a temp file owned by the test.
    let rc = unsafe {
        nix::libc::setxattr(
            c_path.as_ptr(),
            attr_name.as_ptr(),
            attr_value.as_ptr().cast(),
            target_host_path.len(),
            0,
        )
    };
    assert_eq!(rc, 0, "setxattr should succeed for test portal path");

    assert_eq!(
        read_document_portal_host_path_xattr(&portal_file.to_string_lossy()),
        Some(target_host_path.to_string())
    );
    assert_eq!(
        read_document_portal_host_path_xattr_bytes(&portal_file.to_string_lossy()),
        Some(target_host_path.as_bytes().to_vec())
    );
}

#[cfg(target_os = "linux")]
#[test]
fn read_document_portal_host_path_xattr_bytes_returns_raw_value() {
    let temp_dir = tempdir().unwrap();
    let portal_file = temp_dir.path().join("game.exe");
    std::fs::write(&portal_file, b"test").unwrap();

    let raw_value: &[u8] = b"/home/user/game.exe \n\0";
    let c_path = CString::new(portal_file.to_string_lossy().as_bytes()).unwrap();
    let attr_name = CStr::from_bytes_with_nul(DOCUMENT_PORTAL_HOST_PATH_XATTR).unwrap();

    // SAFETY: `c_path` and `attr_name` are valid NUL-terminated strings and
    // `raw_value` is a live slice of the given length for the whole call.
    let rc = unsafe {
        nix::libc::setxattr(
            c_path.as_ptr(),
            attr_name.as_ptr(),
            raw_value.as_ptr().cast(),
            raw_value.len(),
            0,
        )
    };
    assert_eq!(rc, 0, "setxattr should succeed for test portal path");

    let path = portal_file.to_string_lossy();
    assert_eq!(
        read_document_portal_host_path_xattr_bytes(&path),
        Some(raw_value.to_vec())
    );
    // The lossy form strips one trailing NUL, then trims whitespace.
    assert_eq!(
        read_document_portal_host_path_xattr(&path),
        Some("/home/user/game.exe".to_string())
    );
}

#[test]
fn nested_document_portal_host_path_appends_rest_only_when_missing() {
    let rest = ["sub", "game.exe"];
    // Correct per-inode xattr: already ends with the remainder.
    assert_eq!(
        nested_document_portal_host_path(Path::new("/home/user/Games/sub/game.exe"), &rest),
        Path::new("/home/user/Games/sub/game.exe")
    );
    // xdg-desktop-portal 1.19.0-1.19.2: the exported entry's path.
    assert_eq!(
        nested_document_portal_host_path(Path::new("/home/user/Games"), &rest),
        Path::new("/home/user/Games/sub/game.exe")
    );
    // Component-wise match, not a string suffix.
    assert_eq!(
        nested_document_portal_host_path(Path::new("/home/user/Gamessub/game.exe"), &rest),
        Path::new("/home/user/Gamessub/game.exe/sub/game.exe")
    );
    // A pick of the exported entry itself has no remainder.
    assert_eq!(
        nested_document_portal_host_path(Path::new("/home/user/game.exe"), &[]),
        Path::new("/home/user/game.exe")
    );
}
