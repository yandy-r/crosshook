//! Community import naming: an import never replaces anything at `<name>.toml`; it takes the
//! first free `<name>-N` and fails with `AlreadyExists` when none is left.

use std::fs;
use std::path::{Path, PathBuf};

use tempfile::tempdir;

use super::{import_community_profile, CommunityExchangeError};
use crate::profile::{CommunityProfileManifest, CommunityProfileMetadata, GameProfile};

/// Writes a manifest for "Elden Ring" (derived profile name `elden-ring`) and returns its path.
fn write_manifest(dir: &Path) -> PathBuf {
    let metadata = CommunityProfileMetadata {
        game_name: "Elden Ring".to_string(),
        ..CommunityProfileMetadata::default()
    };
    let manifest = CommunityProfileManifest::new(metadata, GameProfile::default());
    let mut value = serde_json::to_value(&manifest).unwrap();
    value["schema_version"] = manifest.schema_version.into();
    let path = dir.join("tap").join("community.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, serde_json::to_string(&value).unwrap()).unwrap();
    path
}

#[test]
fn community_import_never_overwrites_an_existing_profile() {
    let temp_dir = tempdir().unwrap();
    let manifest = write_manifest(temp_dir.path());
    let import_dir = temp_dir.path().join("imported");
    let first = import_community_profile(&manifest, &import_dir).unwrap();
    assert_eq!(first.profile_name, "elden-ring");
    let edited = "# user edits\n[game]\nname = \"My Edited Elden Ring\"\n";
    fs::write(&first.profile_path, edited).unwrap();

    let second = import_community_profile(&manifest, &import_dir).unwrap();

    assert_eq!(fs::read_to_string(&first.profile_path).unwrap(), edited);
    assert_eq!(second.profile_name, "elden-ring-2");
    assert_eq!(second.profile_path, import_dir.join("elden-ring-2.toml"));
    assert!(second.profile_path.exists());
    let third = import_community_profile(&manifest, &import_dir).unwrap();
    assert_eq!(third.profile_name, "elden-ring-3");
}

#[cfg(unix)]
#[test]
fn community_import_treats_a_dangling_symlink_as_taken() {
    let temp_dir = tempdir().unwrap();
    let manifest = write_manifest(temp_dir.path());
    let import_dir = temp_dir.path().join("imported");
    fs::create_dir_all(&import_dir).unwrap();
    let link = import_dir.join("elden-ring.toml");
    std::os::unix::fs::symlink(temp_dir.path().join("missing-target.toml"), &link).unwrap();

    let imported = import_community_profile(&manifest, &import_dir).unwrap();

    assert_eq!(imported.profile_name, "elden-ring-2");
    assert!(fs::symlink_metadata(&link)
        .unwrap()
        .file_type()
        .is_symlink());
    assert!(!temp_dir.path().join("missing-target.toml").exists());
}

#[test]
fn community_import_fails_with_no_free_name_when_every_name_is_taken() {
    let temp_dir = tempdir().unwrap();
    let manifest = write_manifest(temp_dir.path());
    let import_dir = temp_dir.path().join("imported");
    fs::create_dir_all(&import_dir).unwrap();
    fs::write(import_dir.join("elden-ring.toml"), "").unwrap();
    for suffix in 2..=1000 {
        fs::write(import_dir.join(format!("elden-ring-{suffix}.toml")), "").unwrap();
    }

    let error = import_community_profile(&manifest, &import_dir).unwrap_err();

    assert_eq!(
        error,
        CommunityExchangeError::NoFreeName {
            base: "elden-ring".to_string(),
            max_suffix: 1000,
        }
    );
    assert_eq!(
        error.to_string(),
        "no free profile name for 'elden-ring' (tried up to 'elden-ring-1000')"
    );
    assert!(!import_dir.join("elden-ring-1001.toml").exists());
    assert_eq!(
        fs::read_to_string(import_dir.join("elden-ring.toml")).unwrap(),
        ""
    );
}
