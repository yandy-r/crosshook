use super::super::community_schema::{
    CommunityProfileManifest, CommunityProfileMetadata, CompatibilityRating,
};
use super::super::GameProfile;
use super::error::CommunityExchangeError;
use crate::metadata::hash_trainer_file;
use crate::offline::normalize_sha256_hex;
use crate::steam::proton::resolve_proton_path;
use crate::steam::{
    attempt_auto_populate, discover_steam_root_candidates, SteamAutoPopulateFieldState,
    SteamAutoPopulateRequest,
};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

/// Clears filesystem-specific paths from a profile so a community JSON manifest does not embed
/// the exporter's machine layout. Non-path hints (game name, Steam app id, launch method, etc.)
/// are preserved.
pub(super) fn sanitize_profile_for_community_export(profile: &GameProfile) -> GameProfile {
    let mut out = profile.portable_profile();
    out.injection.dll_paths.clear();
    out.steam.launcher.icon_path.clear();
    out.runtime.proton_path.clear();
    out.runtime.working_directory.clear();

    // Guard against local file paths leaking through the base game section.
    // portable_profile() already clears local_override via Default::default(), but these
    // explicit clears ensure no machine-local art path survives export even if a path was
    // written directly into the base section.
    out.game.custom_cover_art_path.clear();
    out.game.custom_portrait_art_path.clear();
    out.game.custom_background_art_path.clear();

    out
}

pub(super) fn hydrate_imported_profile(profile: &GameProfile) -> GameProfile {
    let mut hydrated = profile.effective_profile();

    let game_path = hydrated.game.executable_path.trim();
    if !game_path.is_empty() {
        let request = SteamAutoPopulateRequest {
            game_path: PathBuf::from(game_path),
            steam_client_install_path: PathBuf::new(),
        };
        let auto = attempt_auto_populate(&request);

        if hydrated.steam.app_id.trim().is_empty()
            && matches!(auto.app_id_state, SteamAutoPopulateFieldState::Found)
            && !auto.app_id.trim().is_empty()
        {
            hydrated.steam.app_id = auto.app_id;
        }

        if hydrated.steam.compatdata_path.trim().is_empty()
            && matches!(auto.compatdata_state, SteamAutoPopulateFieldState::Found)
            && !auto.compatdata_path.as_os_str().is_empty()
        {
            hydrated.steam.compatdata_path = auto.compatdata_path.to_string_lossy().into_owned();
        }

        if hydrated.steam.proton_path.trim().is_empty()
            && matches!(auto.proton_state, SteamAutoPopulateFieldState::Found)
            && !auto.proton_path.as_os_str().is_empty()
        {
            hydrated.steam.proton_path = auto.proton_path.to_string_lossy().into_owned();
        }
    }

    hydrate_from_steam_app_id(&mut hydrated);

    if hydrated.runtime.prefix_path.trim().is_empty()
        && !hydrated.steam.compatdata_path.trim().is_empty()
    {
        hydrated.runtime.prefix_path = hydrated.steam.compatdata_path.clone();
    }

    hydrated
}

fn hydrate_from_steam_app_id(profile: &mut GameProfile) {
    let steam_app_id = profile.steam.app_id.trim().to_string();
    if steam_app_id.is_empty() {
        return;
    }

    let mut diagnostics = Vec::new();
    let steam_roots = discover_steam_root_candidates("", &mut diagnostics);

    if profile.steam.compatdata_path.trim().is_empty() {
        for steam_root in &steam_roots {
            let candidate = steam_root
                .join("steamapps")
                .join("compatdata")
                .join(&steam_app_id);
            if candidate.is_dir() {
                profile.steam.compatdata_path = candidate.to_string_lossy().into_owned();
                break;
            }
        }
    }

    if profile.steam.proton_path.trim().is_empty() {
        let proton = resolve_proton_path(&steam_app_id, &steam_roots, &mut diagnostics);
        if matches!(proton.state, SteamAutoPopulateFieldState::Found)
            && !proton.proton_path.as_os_str().is_empty()
        {
            profile.steam.proton_path = proton.proton_path.to_string_lossy().into_owned();
        }
    }
}

pub(super) fn build_metadata(profile: &GameProfile) -> CommunityProfileMetadata {
    let trainer_sha256 = {
        let p = profile.trainer.path.trim();
        if !p.is_empty() {
            hash_trainer_file(Path::new(p))
        } else {
            None
        }
    }
    .or_else(|| {
        let c = profile.trainer.community_trainer_sha256.trim();
        if c.is_empty() {
            None
        } else {
            normalize_sha256_hex(c)
        }
    });

    CommunityProfileMetadata {
        game_name: profile.game.name.clone(),
        game_version: String::new(),
        trainer_name: derive_display_name(&profile.trainer.path),
        trainer_version: String::new(),
        proton_version: String::new(),
        platform_tags: Vec::new(),
        compatibility_rating: CompatibilityRating::Unknown,
        author: String::new(),
        description: String::new(),
        trainer_sha256,
    }
}

pub(super) fn derive_import_name(manifest: &CommunityProfileManifest, json_path: &Path) -> String {
    let game_name = manifest.metadata.game_name.trim();
    if !game_name.is_empty() {
        return sanitize_profile_name(game_name);
    }

    json_path
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
        .map(sanitize_profile_name)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "community-profile".to_string())
}

fn sanitize_profile_name(name: &str) -> String {
    let mut slug = String::with_capacity(name.len());
    let mut last_was_separator = false;

    for ch in name.trim().chars() {
        if ch.is_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_was_separator = false;
        } else if !last_was_separator {
            slug.push('-');
            last_was_separator = true;
        }
    }

    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "community-profile".to_string()
    } else {
        slug
    }
}

fn derive_display_name(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|value| value.to_str())
        .map(str::trim)
        .unwrap_or_default()
        .to_string()
}

pub(super) fn write_manifest(
    output_path: &Path,
    manifest: &CommunityProfileManifest,
) -> Result<(), CommunityExchangeError> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| CommunityExchangeError::Io {
            action: "create the community profile export directory".to_string(),
            path: parent.to_path_buf(),
            message: error.to_string(),
        })?;
    }

    let mut value =
        serde_json::to_value(manifest).map_err(|error| CommunityExchangeError::Json {
            path: output_path.to_path_buf(),
            message: error.to_string(),
        })?;

    let Some(object) = value.as_object_mut() else {
        return Err(CommunityExchangeError::InvalidManifest {
            message: "community profile manifest must serialize as a JSON object".to_string(),
        });
    };
    object.insert(
        "schema_version".to_string(),
        Value::from(manifest.schema_version),
    );

    let json =
        serde_json::to_string_pretty(&value).map_err(|error| CommunityExchangeError::Json {
            path: output_path.to_path_buf(),
            message: error.to_string(),
        })?;

    fs::write(output_path, json).map_err(|error| CommunityExchangeError::Io {
        action: "write the community profile JSON".to_string(),
        path: output_path.to_path_buf(),
        message: error.to_string(),
    })?;

    Ok(())
}
