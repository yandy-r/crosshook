use std::ffi::{CStr, CString};
use std::fs;

use tempfile::tempdir;

use super::super::detect::{
    is_flatpak_with, normalize_flatpak_host_path, read_document_portal_host_path_xattr,
    DOCUMENT_PORTAL_HOST_PATH_XATTR,
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
}
