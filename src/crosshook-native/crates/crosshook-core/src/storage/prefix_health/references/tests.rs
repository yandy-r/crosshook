#![cfg(unix)]

use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use super::collect_profile_prefix_references;
use crate::launch::request::{METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH};
use crate::profile::{GameProfile, ProfileStore};
use crate::storage::prefix_health::{
    cleanup_prefix_storage, scan_prefix_storage, PrefixCleanupTarget, PrefixCleanupTargetKind,
    PrefixReferenceSource, PrefixStorageScanResult,
};

/// Creates `<pfx>/drive_c/CrossHook/StagedTrainers`, the CrossHook-managed marker that
/// trainer staging leaves in any prefix it touched (Steam-owned ones included).
fn make_managed_prefix(pfx: &Path) -> PathBuf {
    fs::create_dir_all(pfx.join("drive_c/CrossHook/StagedTrainers")).unwrap();
    fs::canonicalize(pfx).unwrap()
}

fn path_str(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn steam_profile(compatdata: &Path) -> GameProfile {
    let mut profile = GameProfile::default();
    profile.launch.method = METHOD_STEAM_APPLAUNCH.to_string();
    profile.steam.enabled = true;
    profile.steam.app_id = "111".to_string();
    profile.steam.compatdata_path = path_str(compatdata);
    profile
}

fn proton_profile(prefix: &Path) -> GameProfile {
    let mut profile = GameProfile::default();
    profile.launch.method = METHOD_PROTON_RUN.to_string();
    profile.runtime.prefix_path = path_str(prefix);
    profile
}

fn orphan_target(pfx: &Path) -> PrefixCleanupTarget {
    PrefixCleanupTarget {
        kind: PrefixCleanupTargetKind::OrphanPrefix,
        resolved_prefix_path: path_str(pfx),
        target_path: path_str(pfx),
    }
}

/// `steamapps/compatdata/{111,222}/pfx`, both carrying the CrossHook marker.
struct Fixture {
    tmp: TempDir,
    store: ProfileStore,
    steam_compatdata: PathBuf,
    steam_pfx: PathBuf,
    sibling_pfx: PathBuf,
}

fn fixture() -> Fixture {
    let tmp = TempDir::new().unwrap();
    let compatdata_root = tmp.path().join("steamapps/compatdata");
    let steam_compatdata = compatdata_root.join("111");
    let steam_pfx = make_managed_prefix(&steam_compatdata.join("pfx"));
    let sibling_pfx = make_managed_prefix(&compatdata_root.join("222/pfx"));
    let store = ProfileStore::with_base_path(tmp.path().join("profiles"));
    Fixture {
        tmp,
        store,
        steam_compatdata,
        steam_pfx,
        sibling_pfx,
    }
}

fn scan(store: &ProfileStore) -> (Vec<super::PrefixReference>, PrefixStorageScanResult) {
    let references = collect_profile_prefix_references(store).unwrap();
    assert!(!references.profiles_load_failed);
    let result = scan_prefix_storage(&references.references, 14, false).unwrap();
    (references.references, result)
}

fn referencing_profiles(store: &ProfileStore, pfx: &Path) -> Vec<String> {
    collect_profile_prefix_references(store)
        .unwrap()
        .references
        .into_iter()
        .filter(|reference| reference.resolved_prefix_path == path_str(pfx))
        .map(|reference| reference.profile_name)
        .collect()
}

#[test]
fn steam_profile_with_only_compatdata_path_protects_its_prefix_from_orphan_cleanup() {
    let fx = fixture();
    fx.store
        .save("sibling", &proton_profile(&fx.sibling_pfx))
        .unwrap();
    fx.store
        .save("steam-game", &steam_profile(&fx.steam_compatdata))
        .unwrap();

    let (references, result) = scan(&fx.store);

    let steam_entry = result
        .prefixes
        .iter()
        .find(|entry| entry.resolved_prefix_path == path_str(&fx.steam_pfx))
        .expect("steam prefix is scanned");
    assert!(!steam_entry.is_orphan);
    assert_eq!(steam_entry.referenced_by_profiles, vec!["steam-game"]);
    assert!(
        result.orphan_targets.is_empty(),
        "unexpected orphans: {:?}",
        result.orphan_targets
    );

    // A forged orphan target for the Steam prefix is refused too.
    let cleanup = cleanup_prefix_storage(&references, &[orphan_target(&fx.steam_pfx)]);
    assert!(cleanup.deleted.is_empty(), "deleted: {:?}", cleanup.deleted);
    assert!(fx.steam_pfx.join("drive_c").is_dir());
}

#[test]
fn steam_compatdata_reference_never_makes_sibling_steam_prefixes_deletable() {
    let fx = fixture();
    fx.store
        .save("steam-game", &steam_profile(&fx.steam_compatdata))
        .unwrap();

    let (references, result) = scan(&fx.store);
    assert!(
        result.orphan_targets.is_empty(),
        "unexpected orphans: {:?}",
        result.orphan_targets
    );

    let cleanup = cleanup_prefix_storage(&references, &[orphan_target(&fx.sibling_pfx)]);
    assert!(cleanup.deleted.is_empty(), "deleted: {:?}", cleanup.deleted);
    assert!(fx.sibling_pfx.join("drive_c").is_dir());
}

#[test]
fn runtime_prefix_mirroring_compatdata_stays_protect_only() {
    let fx = fixture();
    // Community imports copy `steam.compatdata_path` into `runtime.prefix_path`.
    let mut profile = steam_profile(&fx.steam_compatdata);
    profile.runtime.prefix_path = path_str(&fx.steam_compatdata.join("pfx"));
    fx.store.save("steam-game", &profile).unwrap();

    let (references, result) = scan(&fx.store);
    assert_eq!(references.len(), 1, "references: {references:?}");
    assert_eq!(
        references[0].source,
        PrefixReferenceSource::SteamCompatdataPath
    );
    assert!(
        result.orphan_targets.is_empty(),
        "unexpected orphans: {:?}",
        result.orphan_targets
    );

    let cleanup = cleanup_prefix_storage(&references, &[orphan_target(&fx.sibling_pfx)]);
    assert!(cleanup.deleted.is_empty(), "deleted: {:?}", cleanup.deleted);
    assert!(fx.sibling_pfx.join("drive_c").is_dir());
}

#[test]
fn steam_profile_references_both_compatdata_and_runtime_prefix() {
    let fx = fixture();
    let runtime_pfx = make_managed_prefix(&fx.tmp.path().join("prefixes/game/pfx"));
    let mut profile = steam_profile(&fx.steam_compatdata);
    profile.runtime.prefix_path = path_str(&runtime_pfx);
    fx.store.save("steam-game", &profile).unwrap();

    let references = collect_profile_prefix_references(&fx.store)
        .unwrap()
        .references;
    let sources: Vec<_> = references
        .iter()
        .map(|reference| (reference.resolved_prefix_path.clone(), reference.source))
        .collect();
    assert_eq!(
        sources,
        vec![
            (
                path_str(&runtime_pfx),
                PrefixReferenceSource::RuntimePrefixPath
            ),
            (
                path_str(&fx.steam_pfx),
                PrefixReferenceSource::SteamCompatdataPath
            ),
        ]
    );
}

#[test]
fn proton_profile_compatdata_path_is_still_referenced() {
    let fx = fixture();
    let mut profile = GameProfile::default();
    profile.launch.method = METHOD_PROTON_RUN.to_string();
    profile.steam.compatdata_path = path_str(&fx.steam_compatdata);
    fx.store.save("proton-game", &profile).unwrap();

    assert_eq!(
        referencing_profiles(&fx.store, &fx.steam_pfx),
        vec!["proton-game"]
    );
}
