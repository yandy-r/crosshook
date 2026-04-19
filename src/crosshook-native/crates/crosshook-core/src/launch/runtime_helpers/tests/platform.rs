use std::path::Path;

use crate::launch::runtime_helpers::environment::resolve_flatpak_host_dbus_session_bus_address_with;
use crate::launch::runtime_helpers::platform::{
    flatpak_host_umu_candidates, probe_flatpak_host_umu_candidates, resolve_umu_run_path,
};

#[test]
fn resolve_umu_run_path_returns_none_when_no_umu_run_present() {
    let dir = tempfile::tempdir().unwrap();
    // empty directory — no umu-run binary
    let _guard = crate::launch::test_support::ScopedCommandSearchPath::new(dir.path());
    assert!(resolve_umu_run_path().is_none());
}

#[test]
fn resolve_umu_run_path_returns_path_when_executable_present() {
    let dir = tempfile::tempdir().unwrap();
    let umu_stub = dir.path().join("umu-run");
    std::fs::write(&umu_stub, "#!/bin/sh\nexit 0\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&umu_stub, std::fs::Permissions::from_mode(0o755)).unwrap();

    let _guard = crate::launch::test_support::ScopedCommandSearchPath::new(dir.path());
    let resolved = resolve_umu_run_path();
    assert!(resolved.is_some(), "expected Some(path), got None");
    assert!(resolved.unwrap().ends_with("/umu-run"));
}

#[test]
fn resolve_umu_run_path_returns_none_when_file_not_executable() {
    let dir = tempfile::tempdir().unwrap();
    let umu_stub = dir.path().join("umu-run");
    std::fs::write(&umu_stub, "not a real executable\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&umu_stub, std::fs::Permissions::from_mode(0o644)).unwrap();

    let _guard = crate::launch::test_support::ScopedCommandSearchPath::new(dir.path());
    assert!(resolve_umu_run_path().is_none());
}

#[test]
fn flatpak_host_umu_candidates_includes_home_and_var_home_paths() {
    let home = Path::new("/home/tester");
    let candidates = flatpak_host_umu_candidates(Some(home), Some("tester"));
    let rendered: Vec<String> = candidates
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    for expected in [
        "/home/tester/.local/bin/umu-run",
        "/home/tester/.local/share/umu/umu-run",
        "/home/tester/.local/pipx/venvs/umu-launcher/bin/umu-run",
        "/run/host/home/tester/.local/bin/umu-run",
        "/var/home/tester/.local/bin/umu-run",
        "/run/host/var/home/tester/.local/bin/umu-run",
    ] {
        assert!(
            rendered.iter().any(|candidate| candidate == expected),
            "expected candidate list to include {expected}, got: {rendered:?}"
        );
    }
}

#[test]
fn probe_flatpak_host_umu_candidates_prefers_home_local_bin() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let home = tmp.path().join("home");
    std::fs::create_dir_all(home.join(".local/bin")).expect("create .local/bin");
    let umu_stub = home.join(".local/bin/umu-run");
    std::fs::write(&umu_stub, "#!/bin/sh\nexit 0\n").expect("write umu-run stub");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&umu_stub, std::fs::Permissions::from_mode(0o755))
            .expect("chmod umu-run stub");
    }

    let resolved = probe_flatpak_host_umu_candidates(Some(home.as_path()), Some("tester"));
    assert_eq!(
        resolved.as_deref(),
        Some(umu_stub.to_string_lossy().as_ref())
    );
}

#[test]
fn resolve_flatpak_host_dbus_session_bus_address_rewrites_flatpak_bus_to_host_bus() {
    let resolved = resolve_flatpak_host_dbus_session_bus_address_with(
        "unix:path=/run/flatpak/bus",
        "/run/user/1000",
        |path| path == "/run/user/1000/bus",
    );

    assert_eq!(resolved, "unix:path=/run/user/1000/bus");
}

#[test]
fn resolve_flatpak_host_dbus_session_bus_address_preserves_suffix_when_rewritten() {
    let resolved = resolve_flatpak_host_dbus_session_bus_address_with(
        "unix:path=/run/flatpak/bus,guid=abc123",
        "/run/user/1000",
        |path| path == "/run/user/1000/bus",
    );

    assert_eq!(resolved, "unix:path=/run/user/1000/bus,guid=abc123");
}

#[test]
fn resolve_flatpak_host_dbus_session_bus_address_preserves_host_visible_bus() {
    let resolved = resolve_flatpak_host_dbus_session_bus_address_with(
        "unix:path=/run/user/1000/bus",
        "/run/user/1000",
        |path| path == "/run/user/1000/bus",
    );

    assert_eq!(resolved, "unix:path=/run/user/1000/bus");
}

#[test]
fn resolve_flatpak_host_dbus_session_bus_address_drops_missing_bus_paths() {
    let resolved = resolve_flatpak_host_dbus_session_bus_address_with(
        "unix:path=/run/flatpak/bus",
        "/run/user/1000",
        |_| false,
    );

    assert_eq!(resolved, "");
}

#[test]
fn resolve_flatpak_host_dbus_session_bus_address_preserves_guid_suffix_on_visible_bus() {
    let resolved = resolve_flatpak_host_dbus_session_bus_address_with(
        "unix:path=/run/user/1000/bus,guid=abc123",
        "/run/user/1000",
        |path| path == "/run/user/1000/bus",
    );

    assert_eq!(resolved, "unix:path=/run/user/1000/bus,guid=abc123");
}
