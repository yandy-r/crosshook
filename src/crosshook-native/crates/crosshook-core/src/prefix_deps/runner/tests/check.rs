use std::fs;

use tempfile::tempdir;

use super::make_fake_binary;
use crate::prefix_deps::runner::check::build_check_command_with;
use crate::prefix_deps::runner::check_installed;
use crate::prefix_deps::PrefixDepsTool;

#[tokio::test]
async fn check_installed_parses_whitespace_output() {
    let tmp = tempdir().unwrap();
    let binary = make_fake_binary(
        tmp.path(),
        "winetricks",
        "#!/bin/sh\necho 'vcrun2019 dotnet48'\n",
    );
    // Create a fake prefix directory (resolve_wine_prefix_path will look for pfx/).
    let pfx = tmp.path().join("pfx");
    fs::create_dir_all(&pfx).unwrap();

    let result = check_installed(
        &binary,
        tmp.path().to_str().unwrap(),
        PrefixDepsTool::Winetricks,
        None,
    )
    .await;
    assert!(result.is_ok(), "error: {:?}", result.err());
    let packages = result.unwrap();
    assert_eq!(packages, vec!["vcrun2019", "dotnet48"]);
}

/// Full argv (program first) of a built command.
fn argv(command: &tokio::process::Command) -> Vec<String> {
    let std = command.as_std();
    std::iter::once(std.get_program())
        .chain(std.get_args())
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}

#[test]
fn check_command_flatpak_argv_has_watch_bus_after_host() {
    let tmp = tempdir().unwrap();
    fs::create_dir_all(tmp.path().join("pfx")).unwrap();
    let binary = "/opt/tools/check-tool";

    let command = build_check_command_with(
        binary,
        tmp.path().to_str().unwrap(),
        PrefixDepsTool::Winetricks,
        None,
        true,
    )
    .unwrap();

    let argv = argv(&command);
    assert_eq!(argv[0..3], ["flatpak-spawn", "--host", "--watch-bus"]);
    assert_eq!(argv[argv.len() - 2..], [binary, "list-installed"]);
    let expected_prefix = fs::canonicalize(tmp.path().join("pfx")).unwrap();
    assert!(
        argv.contains(&format!("--env=WINEPREFIX={}", expected_prefix.display())),
        "missing WINEPREFIX env arg: {argv:?}"
    );
}

#[test]
fn check_command_flatpak_protontricks_argv_has_app_id_before_list_installed() {
    let tmp = tempdir().unwrap();
    fs::create_dir_all(tmp.path().join("pfx")).unwrap();
    let binary = "/opt/tools/check-tool";

    let command = build_check_command_with(
        binary,
        tmp.path().to_str().unwrap(),
        PrefixDepsTool::Protontricks,
        Some("12345"),
        true,
    )
    .unwrap();

    let argv = argv(&command);
    assert_eq!(argv[0..3], ["flatpak-spawn", "--host", "--watch-bus"]);
    assert_eq!(argv[argv.len() - 3..], [binary, "12345", "list-installed"]);
}

#[test]
fn check_command_native_runs_binary_directly() {
    let tmp = tempdir().unwrap();
    fs::create_dir_all(tmp.path().join("pfx")).unwrap();

    let command = build_check_command_with(
        "/opt/tools/check-tool",
        tmp.path().to_str().unwrap(),
        PrefixDepsTool::Winetricks,
        None,
        false,
    )
    .unwrap();

    assert_eq!(argv(&command), ["/opt/tools/check-tool", "list-installed"]);
}

#[test]
fn check_command_protontricks_requires_app_id() {
    let tmp = tempdir().unwrap();
    fs::create_dir_all(tmp.path().join("pfx")).unwrap();

    let result = build_check_command_with(
        "/opt/tools/check-tool",
        tmp.path().to_str().unwrap(),
        PrefixDepsTool::Protontricks,
        None,
        true,
    );

    assert!(matches!(
        result,
        Err(crate::prefix_deps::PrefixDepsError::ValidationError(_))
    ));
}
