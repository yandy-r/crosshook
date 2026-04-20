use crosshook_core::profile::{GameProfile, ProfileStore};
use crosshook_core::steam::libraries::discover_steam_libraries;
use crosshook_core::steam::manifest::parse_manifest_full;
use crosshook_core::steam::{discover_steam_root_candidates, SteamLibrary};
use std::collections::HashMap;

/// Same derivation as `commands/profile.rs::derive_steam_client_install_path` — profiles
/// store Proton compatdata, not the Steam client install path directly.
pub(super) fn steam_client_install_path_from_profile(profile: &GameProfile) -> String {
    const STEAM_COMPATDATA_MARKER: &str = "/steamapps/compatdata/";
    let compatdata_path = profile.steam.compatdata_path.trim().replace('\\', "/");
    compatdata_path
        .split_once(STEAM_COMPATDATA_MARKER)
        .map(|(steam_root, _)| steam_root.to_string())
        .unwrap_or_default()
}

fn manifest_build_id_from_libraries(libraries: &[SteamLibrary], app_id: &str) -> Option<String> {
    for library in libraries {
        let candidate = library
            .steamapps_path
            .join(format!("appmanifest_{app_id}.acf"));
        if candidate.is_file() {
            if let Ok(data) = parse_manifest_full(&candidate) {
                if !data.build_id.is_empty() {
                    return Some(data.build_id);
                }
            }
        }
    }
    None
}

/// Live Steam `buildid` from the installed app manifest for this profile's App ID.
pub(super) fn live_steam_build_id_for_profile(profile: &GameProfile) -> Option<String> {
    let app_id = profile.steam.app_id.trim();
    if app_id.is_empty() {
        return None;
    }
    let mut diagnostics = Vec::new();
    let configured = steam_client_install_path_from_profile(profile);
    let steam_roots = discover_steam_root_candidates(
        if configured.is_empty() {
            ""
        } else {
            configured.as_str()
        },
        &mut diagnostics,
    );
    let libraries = discover_steam_libraries(&steam_roots, &mut diagnostics);
    for entry in &diagnostics {
        tracing::debug!(entry, "health steam discovery diagnostic");
    }
    manifest_build_id_from_libraries(&libraries, app_id)
}

/// Resolve live build IDs for many profiles, running Steam discovery once per distinct
/// configured Steam client install path.
pub(super) fn live_steam_build_ids_for_profiles(
    profile_store: &ProfileStore,
    profile_names: &[String],
) -> HashMap<String, Option<String>> {
    let mut by_steam_path: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for name in profile_names {
        let Ok(profile) = profile_store.load(name) else {
            continue;
        };
        let app_id = profile.steam.app_id.trim().to_string();
        if app_id.is_empty() {
            continue;
        }
        let path_key = steam_client_install_path_from_profile(&profile);
        by_steam_path
            .entry(path_key)
            .or_default()
            .push((name.clone(), app_id));
    }

    let mut out: HashMap<String, Option<String>> = HashMap::new();
    for (steam_path, entries) in by_steam_path {
        let mut diagnostics = Vec::new();
        let steam_roots = discover_steam_root_candidates(
            if steam_path.is_empty() {
                ""
            } else {
                steam_path.as_str()
            },
            &mut diagnostics,
        );
        let libraries = discover_steam_libraries(&steam_roots, &mut diagnostics);
        for entry in &diagnostics {
            tracing::debug!(entry, "health batch steam discovery diagnostic");
        }
        for (profile_name, app_id) in entries {
            out.insert(
                profile_name,
                manifest_build_id_from_libraries(&libraries, &app_id),
            );
        }
    }
    out
}
