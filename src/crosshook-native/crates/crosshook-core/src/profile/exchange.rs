use super::community_schema::{
    CommunityProfileManifest, CommunityProfileMetadata, CompatibilityRating,
    COMMUNITY_PROFILE_SCHEMA_VERSION,
};
use crate::metadata::hash_trainer_file;
use crate::offline::normalize_sha256_hex;
use crate::profile::{GameProfile, ProfileStore, ProfileStoreError};
use crate::steam::proton::resolve_proton_path;
use crate::steam::{
    attempt_auto_populate, discover_steam_root_candidates, SteamAutoPopulateFieldState,
    SteamAutoPopulateRequest,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommunityExchangeError {
    Io {
        action: String,
        path: PathBuf,
        message: String,
    },
    Json {
        path: PathBuf,
        message: String,
    },
    InvalidManifest {
        message: String,
    },
    UnsupportedSchemaVersion {
        version: u32,
        supported: u32,
    },
    ProfileStore {
        message: String,
    },
}

impl Display for CommunityExchangeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                action,
                path,
                message,
            } => write!(f, "failed to {action} '{}': {message}", path.display()),
            Self::Json { path, message } => {
                write!(
                    f,
                    "failed to parse community profile '{}': {message}",
                    path.display()
                )
            }
            Self::InvalidManifest { message } => write!(f, "invalid community profile: {message}"),
            Self::UnsupportedSchemaVersion { version, supported } => {
                write!(
                    f,
                    "unsupported community profile schema version {version}; supported version is {supported}"
                )
            }
            Self::ProfileStore { message } => write!(f, "{message}"),
        }
    }
}

impl Error for CommunityExchangeError {}

impl From<ProfileStoreError> for CommunityExchangeError {
    fn from(value: ProfileStoreError) -> Self {
        Self::ProfileStore {
            message: value.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommunityImportResult {
    pub profile_name: String,
    pub source_path: PathBuf,
    pub profile_path: PathBuf,
    pub profile: GameProfile,
    pub manifest: CommunityProfileManifest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommunityImportPreview {
    pub profile_name: String,
    pub source_path: PathBuf,
    pub profile: GameProfile,
    pub manifest: CommunityProfileManifest,
    pub required_prefix_deps: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommunityExportResult {
    pub profile_name: String,
    pub output_path: PathBuf,
    pub manifest: CommunityProfileManifest,
}

pub fn import_community_profile(
    json_path: &Path,
    profiles_dir: &Path,
) -> Result<CommunityImportResult, CommunityExchangeError> {
    let preview = preview_community_profile_import(json_path)?;
    let profile_name = preview.profile_name.clone();
    let manifest = preview.manifest.clone();
    let mut profile = preview.profile.clone();
    if let Some(ref h) = manifest.metadata.trainer_sha256 {
        let t = h.trim();
        if !t.is_empty() {
            profile.trainer.community_trainer_sha256 = t.to_string();
        }
    }

    let store = ProfileStore::with_base_path(profiles_dir.to_path_buf());
    store.save(&profile_name, &profile)?;

    Ok(CommunityImportResult {
        profile_name: profile_name.clone(),
        source_path: json_path.to_path_buf(),
        profile_path: profiles_dir.join(format!("{profile_name}.toml")),
        profile,
        manifest,
    })
}

pub fn preview_community_profile_import(
    json_path: &Path,
) -> Result<CommunityImportPreview, CommunityExchangeError> {
    let content = fs::read_to_string(json_path).map_err(|error| CommunityExchangeError::Io {
        action: "read the community profile JSON".to_string(),
        path: json_path.to_path_buf(),
        message: error.to_string(),
    })?;

    let value: Value =
        serde_json::from_str(&content).map_err(|error| CommunityExchangeError::Json {
            path: json_path.to_path_buf(),
            message: error.to_string(),
        })?;

    validate_manifest_value(&value)?;
    let manifest: CommunityProfileManifest =
        serde_json::from_value(value).map_err(|error| CommunityExchangeError::Json {
            path: json_path.to_path_buf(),
            message: error.to_string(),
        })?;

    validate_schema_version(manifest.schema_version)?;

    let profile_name = derive_import_name(&manifest, json_path);
    let required_prefix_deps = manifest.profile.trainer.required_protontricks.clone();
    let hydrated_profile = hydrate_imported_profile(&manifest.profile);

    Ok(CommunityImportPreview {
        profile_name,
        source_path: json_path.to_path_buf(),
        profile: hydrated_profile,
        manifest,
        required_prefix_deps,
    })
}

pub fn export_community_profile(
    profiles_dir: &Path,
    profile_name: &str,
    output_path: &Path,
) -> Result<CommunityExportResult, CommunityExchangeError> {
    validate_schema_version(COMMUNITY_PROFILE_SCHEMA_VERSION)?;

    let store = ProfileStore::with_base_path(profiles_dir.to_path_buf());
    let profile = store.load(profile_name)?;
    // Metadata (e.g. trainer display name) is derived from the on-disk profile before stripping paths.
    let metadata = build_metadata(&profile);
    let shareable_profile = sanitize_profile_for_community_export(&profile);
    let manifest = CommunityProfileManifest::new(metadata, shareable_profile);

    write_manifest(output_path, &manifest)?;

    Ok(CommunityExportResult {
        profile_name: profile_name.to_string(),
        output_path: output_path.to_path_buf(),
        manifest,
    })
}

pub fn validate_manifest_value(value: &Value) -> Result<(), CommunityExchangeError> {
    let root = value
        .as_object()
        .ok_or_else(|| CommunityExchangeError::InvalidManifest {
            message: "manifest must be a JSON object".to_string(),
        })?;

    let schema_version = root
        .get("schema_version")
        .and_then(Value::as_u64)
        .ok_or_else(|| CommunityExchangeError::InvalidManifest {
            message: "manifest must include an integer schema_version".to_string(),
        })? as u32;
    validate_schema_version(schema_version)?;

    let metadata = required_object(root, "metadata")?;
    for field in [
        "game_name",
        "game_version",
        "trainer_name",
        "trainer_version",
        "proton_version",
        "platform_tags",
        "compatibility_rating",
        "author",
        "description",
    ] {
        require_field(metadata, field)?;
    }

    let profile = required_object(root, "profile")?;
    for field in ["game", "trainer", "injection", "steam", "launch"] {
        require_field(profile, field)?;
    }

    Ok(())
}

pub fn validate_schema_version(version: u32) -> Result<(), CommunityExchangeError> {
    if version > COMMUNITY_PROFILE_SCHEMA_VERSION {
        return Err(CommunityExchangeError::UnsupportedSchemaVersion {
            version,
            supported: COMMUNITY_PROFILE_SCHEMA_VERSION,
        });
    }

    Ok(())
}

fn required_object<'a>(
    parent: &'a serde_json::Map<String, Value>,
    field: &str,
) -> Result<&'a serde_json::Map<String, Value>, CommunityExchangeError> {
    parent.get(field).and_then(Value::as_object).ok_or_else(|| {
        CommunityExchangeError::InvalidManifest {
            message: format!("manifest field '{field}' must be a JSON object"),
        }
    })
}

fn require_field(
    parent: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<(), CommunityExchangeError> {
    if parent.contains_key(field) {
        Ok(())
    } else {
        Err(CommunityExchangeError::InvalidManifest {
            message: format!("manifest is missing required field '{field}'"),
        })
    }
}

/// Clears filesystem-specific paths from a profile so a community JSON manifest does not embed
/// the exporter's machine layout. Non-path hints (game name, Steam app id, launch method, etc.)
/// are preserved.
fn sanitize_profile_for_community_export(profile: &GameProfile) -> GameProfile {
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

fn hydrate_imported_profile(profile: &GameProfile) -> GameProfile {
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

fn build_metadata(profile: &GameProfile) -> CommunityProfileMetadata {
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

fn derive_import_name(manifest: &CommunityProfileManifest, json_path: &Path) -> String {
    let game_name = manifest.metadata.game_name.trim();
    if !game_name.is_empty() {
        return sanitize_profile_name(game_name);
    }

    json_path
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
        .map(|name| sanitize_profile_name(name))
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

fn write_manifest(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::GameProfile;
    use tempfile::tempdir;

    fn sample_profile() -> GameProfile {
        GameProfile {
            game: crate::profile::GameSection {
                name: "Elden Ring".to_string(),
                executable_path: "/games/elden-ring/eldenring.exe".to_string(),
                custom_cover_art_path: String::new(),
                custom_portrait_art_path: String::new(),
                custom_background_art_path: String::new(),
            },
            trainer: crate::profile::TrainerSection {
                path: "/trainers/elden-ring.exe".to_string(),
                kind: "fling".to_string(),
                loading_mode: crate::profile::TrainerLoadingMode::SourceDirectory,
                trainer_type: "unknown".to_string(),
                required_protontricks: Vec::new(),
                community_trainer_sha256: String::new(),
            },
            injection: crate::profile::InjectionSection {
                dll_paths: vec!["/dlls/a.dll".to_string(), "/dlls/b.dll".to_string()],
                inject_on_launch: vec![true, false],
            },
            steam: crate::profile::SteamSection {
                enabled: true,
                app_id: "1245620".to_string(),
                compatdata_path: "/steam/compatdata/1245620".to_string(),
                proton_path: "/steam/proton/proton".to_string(),
                launcher: crate::profile::LauncherSection {
                    icon_path: "/icons/elden-ring.png".to_string(),
                    display_name: "Elden Ring".to_string(),
                },
            },
            runtime: crate::profile::RuntimeSection {
                prefix_path: String::new(),
                proton_path: String::new(),
                working_directory: String::new(),
                steam_app_id: String::new(),
            },
            launch: crate::profile::LaunchSection {
                method: "steam_applaunch".to_string(),
                ..Default::default()
            },
            local_override: crate::profile::LocalOverrideSection::default(),
        }
    }

    fn sample_profile_sanitized_for_export() -> GameProfile {
        let mut p = sample_profile();
        p.game.executable_path.clear();
        p.trainer.path.clear();
        p.injection.dll_paths.clear();
        p.steam.compatdata_path.clear();
        p.steam.proton_path.clear();
        p.steam.launcher.icon_path.clear();
        p.runtime.prefix_path.clear();
        p.runtime.proton_path.clear();
        p.runtime.working_directory.clear();
        p
    }

    #[test]
    fn export_strips_machine_specific_paths() {
        let temp_dir = tempdir().unwrap();
        let profiles_dir = temp_dir.path().join("profiles");
        let export_path = temp_dir.path().join("exports").join("elden-ring.json");
        let store = ProfileStore::with_base_path(profiles_dir.clone());
        let profile = sample_profile();
        let expected_shareable = sample_profile_sanitized_for_export();

        store.save("elden-ring", &profile).unwrap();

        let exported = export_community_profile(&profiles_dir, "elden-ring", &export_path).unwrap();
        assert_eq!(exported.manifest.profile, expected_shareable);
        assert_ne!(exported.manifest.profile, profile);
        assert_eq!(exported.manifest.metadata.trainer_name, "elden-ring");

        let json = fs::read_to_string(&export_path).unwrap();
        let value: Value = serde_json::from_str(&json).unwrap();
        let prof = value.get("profile").and_then(Value::as_object).unwrap();
        let game = prof.get("game").and_then(Value::as_object).unwrap();
        assert_eq!(
            game.get("executable_path").and_then(Value::as_str),
            Some("")
        );
        let trainer = prof.get("trainer").and_then(Value::as_object).unwrap();
        assert_eq!(trainer.get("path").and_then(Value::as_str), Some(""));
        let steam = prof.get("steam").and_then(Value::as_object).unwrap();
        assert_eq!(
            steam.get("compatdata_path").and_then(Value::as_str),
            Some("")
        );
        assert_eq!(steam.get("proton_path").and_then(Value::as_str), Some(""));
    }

    #[test]
    fn export_and_import_round_trip_profile() {
        let temp_dir = tempdir().unwrap();
        let profiles_dir = temp_dir.path().join("profiles");
        let export_path = temp_dir.path().join("exports").join("elden-ring.json");
        let store = ProfileStore::with_base_path(profiles_dir.clone());
        let profile = sample_profile();
        let shareable = sample_profile_sanitized_for_export();

        store.save("elden-ring", &profile).unwrap();

        let exported = export_community_profile(&profiles_dir, "elden-ring", &export_path).unwrap();
        assert_eq!(exported.profile_name, "elden-ring");
        assert_eq!(exported.manifest.profile, shareable);
        assert_eq!(exported.manifest.metadata.game_name, "Elden Ring");
        assert_eq!(exported.manifest.metadata.trainer_name, "elden-ring");

        let imported_profiles_dir = temp_dir.path().join("imported-profiles");
        let imported = import_community_profile(&export_path, &imported_profiles_dir).unwrap();
        assert_eq!(imported.profile_name, "elden-ring");
        assert_eq!(imported.manifest.profile, shareable);

        let imported_store = ProfileStore::with_base_path(imported_profiles_dir);
        let mut loaded = imported_store.load("elden-ring").unwrap();
        // Import may hydrate steam.proton_path from a local Steam install when app_id is set;
        // compare the portable shape expected from the exported manifest.
        loaded.steam.proton_path.clear();
        assert_eq!(loaded, shareable);
    }

    #[test]
    fn rejects_future_schema_versions() {
        let value = serde_json::json!({
            "schema_version": COMMUNITY_PROFILE_SCHEMA_VERSION + 1,
            "metadata": {
                "game_name": "Elden Ring",
                "game_version": "",
                "trainer_name": "",
                "trainer_version": "",
                "proton_version": "",
                "platform_tags": [],
                "compatibility_rating": "unknown",
                "author": "",
                "description": ""
            },
            "profile": sample_profile(),
        });

        let error = validate_manifest_value(&value).unwrap_err();
        assert!(matches!(
            error,
            CommunityExchangeError::UnsupportedSchemaVersion { .. }
        ));
    }

    #[test]
    fn rejects_missing_required_manifest_sections() {
        let value = serde_json::json!({
            "schema_version": COMMUNITY_PROFILE_SCHEMA_VERSION,
            "metadata": {
                "game_name": "Elden Ring",
                "game_version": "",
                "trainer_name": "",
                "trainer_version": "",
                "proton_version": "",
                "platform_tags": [],
                "compatibility_rating": "unknown",
                "author": "",
                "description": ""
            }
        });

        let error = validate_manifest_value(&value).unwrap_err();
        assert!(matches!(
            error,
            CommunityExchangeError::InvalidManifest { .. }
        ));
    }

    #[test]
    fn export_clears_all_custom_art_paths_and_preserves_steam_app_id() {
        let temp_dir = tempdir().unwrap();
        let profiles_dir = temp_dir.path().join("profiles");
        let export_path = temp_dir.path().join("exports").join("elden-ring.json");
        let store = ProfileStore::with_base_path(profiles_dir.clone());

        let mut profile = sample_profile();
        profile.game.custom_cover_art_path = "/home/user/.local/cover.png".to_string();
        profile.game.custom_portrait_art_path = "/home/user/.local/portrait.png".to_string();
        profile.game.custom_background_art_path = "/home/user/.local/background.png".to_string();
        profile.local_override.game.custom_cover_art_path =
            "/home/user/.local/cover-override.png".to_string();
        profile.local_override.game.custom_portrait_art_path =
            "/home/user/.local/portrait-override.png".to_string();
        profile.local_override.game.custom_background_art_path =
            "/home/user/.local/background-override.png".to_string();

        store.save("elden-ring", &profile).unwrap();

        let exported = export_community_profile(&profiles_dir, "elden-ring", &export_path).unwrap();
        let exported_profile = &exported.manifest.profile;

        assert!(
            exported_profile.game.custom_cover_art_path.is_empty(),
            "custom_cover_art_path must be cleared on export"
        );
        assert!(
            exported_profile.game.custom_portrait_art_path.is_empty(),
            "custom_portrait_art_path must be cleared on export"
        );
        assert!(
            exported_profile.game.custom_background_art_path.is_empty(),
            "custom_background_art_path must be cleared on export"
        );

        assert_eq!(
            exported_profile.steam.app_id, "1245620",
            "steam.app_id must survive community export"
        );
    }
}
