use tempfile::tempdir;

use super::write_registry_fixture;
use crate::prefix_deps::runner::is_windows_version_verb;
use crate::prefix_deps::runner::snapshot::snapshot_prefix_windows_version;
use crate::prefix_deps::PrefixDepsError;

#[test]
fn snapshots_windows_7_from_prefix_registry() {
    let tmp = tempdir().unwrap();
    let pfx = tmp.path().join("pfx");
    write_registry_fixture(
        &pfx,
        "\"CurrentBuild\"=\"7601\"\n\"CurrentVersion\"=\"6.1\"\n\"ProductName\"=\"Microsoft Windows 7\"",
    );

    assert_eq!(snapshot_prefix_windows_version(&pfx).unwrap(), "win7");
}

#[test]
fn snapshots_windows_10_from_prefix_registry() {
    let tmp = tempdir().unwrap();
    let pfx = tmp.path().join("pfx");
    write_registry_fixture(
        &pfx,
        "\"CurrentBuild\"=\"19045\"\n\"CurrentVersion\"=\"6.3\"\n\"ProductName\"=\"Microsoft Windows 10\"",
    );

    assert_eq!(snapshot_prefix_windows_version(&pfx).unwrap(), "win10");
}

#[test]
fn snapshots_windows_11_from_build_when_product_name_says_windows_10() {
    let tmp = tempdir().unwrap();
    let pfx = tmp.path().join("pfx");
    write_registry_fixture(
        &pfx,
        "\"CurrentBuild\"=\"22000\"\n\"CurrentVersion\"=\"6.3\"\n\"ProductName\"=\"Microsoft Windows 10\"",
    );

    assert_eq!(snapshot_prefix_windows_version(&pfx).unwrap(), "win11");
}

#[test]
fn snapshots_server_2008_r2_before_ambiguous_windows_7_build() {
    let tmp = tempdir().unwrap();
    let pfx = tmp.path().join("pfx");
    write_registry_fixture(
        &pfx,
        "\"CurrentBuild\"=\"7601\"\n\"CurrentVersion\"=\"6.1\"\n\"ProductName\"=\"Microsoft Windows Server 2008 R2\"",
    );

    assert_eq!(snapshot_prefix_windows_version(&pfx).unwrap(), "win2k8r2");
}

#[test]
fn rejects_unknown_prefix_windows_version() {
    let tmp = tempdir().unwrap();
    let pfx = tmp.path().join("pfx");
    write_registry_fixture(
        &pfx,
        "\"CurrentBuild\"=\"1234\"\n\"CurrentVersion\"=\"1.2\"\n\"ProductName\"=\"Unknown Windows\"",
    );

    assert!(matches!(
        snapshot_prefix_windows_version(&pfx),
        Err(PrefixDepsError::ValidationError(_))
    ));
}

#[test]
fn is_windows_version_verb_accepts_only_version_verbs() {
    for verb in ["win7", "win10", "win11", "winxp", "win2k8r2", "vista"] {
        assert!(is_windows_version_verb(verb), "rejected {verb}");
    }
    for verb in ["dotnet48", "vcrun2019", "-q", "", "WIN10", "win12"] {
        assert!(!is_windows_version_verb(verb), "accepted {verb:?}");
    }
}
