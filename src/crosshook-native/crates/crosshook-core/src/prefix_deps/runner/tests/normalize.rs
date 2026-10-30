use std::fs;

use tempfile::tempdir;

use crate::prefix_deps::runner::command::build_prefix_dep_mutation_command;
use crate::prefix_deps::runner::normalize_prefix_path;

#[test]
fn mutation_command_targets_binary_with_resolved_wineprefix_natively() {
    let tmp = tempdir().unwrap();
    let prefix = tmp.path().join("prefix");
    fs::create_dir_all(prefix.join("pfx")).unwrap();
    let (command, resolved_prefix) =
        build_prefix_dep_mutation_command("winetricks", prefix.to_str().unwrap()).unwrap();

    let expected_pfx = fs::canonicalize(prefix.join("pfx")).unwrap();
    assert_eq!(resolved_prefix, expected_pfx);
    assert_eq!(command.as_std().get_program(), "winetricks");
    let wineprefix = command
        .as_std()
        .get_envs()
        .find(|(key, _)| *key == "WINEPREFIX")
        .and_then(|(_, value)| value);
    assert_eq!(wineprefix, Some(expected_pfx.as_os_str()));
}

#[test]
fn normalize_prefix_path_returns_resolved_pfx_directory() {
    let tmp = tempdir().unwrap();
    let pfx = tmp.path().join("pfx");
    fs::create_dir_all(&pfx).unwrap();

    assert_eq!(
        normalize_prefix_path(tmp.path().to_str().unwrap()).unwrap(),
        pfx.to_string_lossy()
    );
    assert_eq!(
        normalize_prefix_path(pfx.to_str().unwrap()).unwrap(),
        pfx.to_string_lossy()
    );
}

#[cfg(unix)]
#[test]
fn normalize_prefix_path_canonicalizes_symlink_aliases() {
    use std::os::unix::fs::symlink;

    let tmp = tempdir().unwrap();
    let prefix = tmp.path().join("prefix");
    let pfx = prefix.join("pfx");
    fs::create_dir_all(&pfx).unwrap();
    let alias = tmp.path().join("alias");
    symlink(&prefix, &alias).unwrap();

    assert_eq!(
        normalize_prefix_path(alias.to_str().unwrap()).unwrap(),
        fs::canonicalize(&pfx).unwrap().to_string_lossy()
    );
}

#[test]
fn normalize_prefix_path_canonicalizes_parent_segments() {
    let tmp = tempdir().unwrap();
    let base = tmp.path().join("base");
    let prefix = base.join("prefix");
    let pfx = prefix.join("pfx");
    fs::create_dir_all(&pfx).unwrap();
    fs::create_dir_all(base.join("nested")).unwrap();
    let alias = base.join("nested").join("..").join("prefix");

    assert_eq!(
        normalize_prefix_path(alias.to_str().unwrap()).unwrap(),
        fs::canonicalize(&pfx).unwrap().to_string_lossy()
    );
}
