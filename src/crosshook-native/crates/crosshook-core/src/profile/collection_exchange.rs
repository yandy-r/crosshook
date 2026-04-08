//! Export and import-preview for `*.crosshook-collection.toml` collection presets.

use std::collections::HashMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::metadata::{MetadataStore, MetadataStoreError};
use crate::profile::collection_schema::{
    CollectionPresetManifest, CollectionPresetProfileDescriptor, COLLECTION_PRESET_SCHEMA_VERSION,
};
use crate::profile::{resolve_art_app_id, GameProfile, ProfileStore, ProfileStoreError};
#[cfg(test)]
use crate::profile::CollectionDefaultsSection;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollectionExchangeError {
    Io {
        action: String,
        path: PathBuf,
        message: String,
    },
    Toml {
        path: PathBuf,
        message: String,
    },
    InvalidManifest {
        message: String,
    },
    UnsupportedSchemaVersion {
        version: String,
        supported: String,
    },
    Metadata {
        message: String,
    },
    ProfileStore {
        message: String,
    },
}

impl Display for CollectionExchangeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                action,
                path,
                message,
            } => write!(f, "failed to {action} '{}': {message}", path.display()),
            Self::Toml { path, message } => {
                write!(
                    f,
                    "failed to parse collection preset '{}': {message}",
                    path.display()
                )
            }
            Self::InvalidManifest { message } => {
                write!(f, "invalid collection preset: {message}")
            }
            Self::UnsupportedSchemaVersion { version, supported } => write!(
                f,
                "unsupported collection preset schema version {version:?}; supported version is {supported:?}"
            ),
            Self::Metadata { message } => write!(f, "{message}"),
            Self::ProfileStore { message } => write!(f, "{message}"),
        }
    }
}

impl Error for CollectionExchangeError {}

impl From<ProfileStoreError> for CollectionExchangeError {
    fn from(value: ProfileStoreError) -> Self {
        Self::ProfileStore {
            message: value.to_string(),
        }
    }
}

impl From<MetadataStoreError> for CollectionExchangeError {
    fn from(value: MetadataStoreError) -> Self {
        Self::Metadata {
            message: value.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectionExportResult {
    pub collection_id: String,
    pub output_path: PathBuf,
    pub manifest: CollectionPresetManifest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectionPresetMatchCandidate {
    pub profile_name: String,
    pub game_name: String,
    pub steam_app_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectionPresetMatchedEntry {
    pub descriptor: CollectionPresetProfileDescriptor,
    pub local_profile_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectionPresetAmbiguousEntry {
    pub descriptor: CollectionPresetProfileDescriptor,
    pub candidates: Vec<CollectionPresetMatchCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectionImportPreview {
    pub source_path: PathBuf,
    pub manifest: CollectionPresetManifest,
    pub matched: Vec<CollectionPresetMatchedEntry>,
    pub ambiguous: Vec<CollectionPresetAmbiguousEntry>,
    pub unmatched: Vec<CollectionPresetProfileDescriptor>,
}

struct LocalMatchIndex {
    steam_to_profiles: HashMap<String, Vec<String>>,
    pair_to_profiles: HashMap<(String, String), Vec<String>>,
    /// Display info for ambiguous candidates.
    profile_display: HashMap<String, (String, String)>,
}

fn build_local_match_index(store: &ProfileStore) -> Result<LocalMatchIndex, CollectionExchangeError> {
    let mut steam_to_profiles: HashMap<String, Vec<String>> = HashMap::new();
    let mut pair_to_profiles: HashMap<(String, String), Vec<String>> = HashMap::new();
    let mut profile_display: HashMap<String, (String, String)> = HashMap::new();

    let names = store.list()?;
    for profile_name in names {
        let profile = match store.load(&profile_name) {
            Ok(p) => p,
            Err(err) => {
                tracing::warn!(
                    profile = %profile_name,
                    error = %err,
                    "skipping profile while building collection import index"
                );
                continue;
            }
        };

        let app_id = resolve_art_app_id(&profile).to_string();
        let game_name = profile.game.name.clone();
        let sha = profile.trainer.community_trainer_sha256.trim().to_string();

        profile_display.insert(
            profile_name.clone(),
            (game_name.clone(), app_id.clone()),
        );

        if !app_id.is_empty() {
            steam_to_profiles
                .entry(app_id)
                .or_default()
                .push(profile_name.clone());
        }

        let gn = profile.game.name.trim();
        if !gn.is_empty() && !sha.is_empty() {
            let key = (gn.to_string(), sha.clone());
            pair_to_profiles
                .entry(key)
                .or_default()
                .push(profile_name.clone());
        }
    }

    Ok(LocalMatchIndex {
        steam_to_profiles,
        pair_to_profiles,
        profile_display,
    })
}

fn candidates_for_names(
    names: &[String],
    display: &HashMap<String, (String, String)>,
) -> Vec<CollectionPresetMatchCandidate> {
    let mut out = Vec::new();
    for name in names {
        let (game_name, steam_app_id) = display
            .get(name)
            .cloned()
            .unwrap_or_else(|| (String::new(), String::new()));
        out.push(CollectionPresetMatchCandidate {
            profile_name: name.clone(),
            game_name,
            steam_app_id,
        });
    }
    out
}

fn classify_descriptor(
    d: &CollectionPresetProfileDescriptor,
    index: &LocalMatchIndex,
) -> MatchClass {
    let steam_key = d.steam_app_id.trim();
    if !steam_key.is_empty() {
        if let Some(names) = index.steam_to_profiles.get(steam_key) {
            match names.len() {
                0 => {}
                1 => {
                    return MatchClass::Matched {
                        profile: names[0].clone(),
                    };
                }
                _ => {
                    return MatchClass::Ambiguous {
                        names: names.clone(),
                    };
                }
            }
        }
    }

    let gn = d.game_name.trim();
    let sha = d.trainer_community_trainer_sha256.trim();
    if !gn.is_empty() && !sha.is_empty() {
        let key = (gn.to_string(), sha.to_string());
        if let Some(names) = index.pair_to_profiles.get(&key) {
            match names.len() {
                0 => {}
                1 => {
                    return MatchClass::Matched {
                        profile: names[0].clone(),
                    };
                }
                _ => {
                    return MatchClass::Ambiguous {
                        names: names.clone(),
                    };
                }
            }
        }
    }

    MatchClass::Unmatched
}

enum MatchClass {
    Matched { profile: String },
    Ambiguous { names: Vec<String> },
    Unmatched,
}

/// Writes a collection preset TOML for the given collection id.
pub fn export_collection_preset_to_toml(
    metadata_store: &MetadataStore,
    profile_store: &ProfileStore,
    collection_id: &str,
    output_path: &Path,
) -> Result<CollectionExportResult, CollectionExchangeError> {
    let rows = metadata_store.list_collections()?;
    let row = rows
        .into_iter()
        .find(|r| r.collection_id == collection_id)
        .ok_or_else(|| CollectionExchangeError::InvalidManifest {
            message: format!("collection not found: {collection_id}"),
        })?;

    let description = row.description.as_ref().and_then(|s| {
        let t = s.trim();
        if t.is_empty() {
            None
        } else {
            Some(t.to_string())
        }
    });

    let defaults = metadata_store
        .get_collection_defaults(collection_id)?
        .filter(|d| !d.is_empty());

    let member_names = metadata_store.list_profiles_in_collection(collection_id)?;
    let mut descriptors = Vec::with_capacity(member_names.len());

    for name in &member_names {
        let profile = profile_store.load(name).map_err(|e| match e {
            ProfileStoreError::NotFound(path) => CollectionExchangeError::InvalidManifest {
                message: format!(
                    "profile file for collection member {name:?} is missing: {}",
                    path.display()
                ),
            },
            other => other.into(),
        })?;

        descriptors.push(descriptor_from_profile(&profile));
    }

    let manifest = CollectionPresetManifest {
        schema_version: COLLECTION_PRESET_SCHEMA_VERSION.to_string(),
        name: row.name,
        description,
        defaults,
        profiles: descriptors,
    };

    write_preset_toml(output_path, &manifest)?;

    Ok(CollectionExportResult {
        collection_id: collection_id.to_string(),
        output_path: output_path.to_path_buf(),
        manifest,
    })
}

fn descriptor_from_profile(profile: &GameProfile) -> CollectionPresetProfileDescriptor {
    CollectionPresetProfileDescriptor {
        steam_app_id: resolve_art_app_id(profile).to_string(),
        game_name: profile.game.name.clone(),
        trainer_community_trainer_sha256: profile.trainer.community_trainer_sha256.clone(),
    }
}

fn write_preset_toml(
    output_path: &Path,
    manifest: &CollectionPresetManifest,
) -> Result<(), CollectionExchangeError> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| CollectionExchangeError::Io {
            action: "create the collection preset export directory".to_string(),
            path: parent.to_path_buf(),
            message: error.to_string(),
        })?;
    }

    let body = toml::to_string_pretty(manifest).map_err(|e| CollectionExchangeError::Toml {
        path: output_path.to_path_buf(),
        message: e.to_string(),
    })?;

    let out = format!(
        "# CrossHook collection preset\n\
         # https://github.com/yandy-r/crosshook\n\
         #\n\
         \n\
         {body}"
    );

    fs::write(output_path, out).map_err(|error| CollectionExchangeError::Io {
        action: "write the collection preset file".to_string(),
        path: output_path.to_path_buf(),
        message: error.to_string(),
    })
}

/// Parses and validates a preset file, then classifies each descriptor against local profiles.
pub fn preview_collection_preset_import(
    profile_store: &ProfileStore,
    path: &Path,
) -> Result<CollectionImportPreview, CollectionExchangeError> {
    let content = fs::read_to_string(path).map_err(|error| CollectionExchangeError::Io {
        action: "read the collection preset file".to_string(),
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;

    let manifest = parse_collection_preset_toml(&content, path)?;
    let index = build_local_match_index(profile_store)?;

    let mut matched = Vec::new();
    let mut ambiguous = Vec::new();
    let mut unmatched = Vec::new();

    for d in &manifest.profiles {
        match classify_descriptor(d, &index) {
            MatchClass::Matched { profile } => {
                matched.push(CollectionPresetMatchedEntry {
                    descriptor: d.clone(),
                    local_profile_name: profile,
                });
            }
            MatchClass::Ambiguous { names } => {
                ambiguous.push(CollectionPresetAmbiguousEntry {
                    descriptor: d.clone(),
                    candidates: candidates_for_names(&names, &index.profile_display),
                });
            }
            MatchClass::Unmatched => unmatched.push(d.clone()),
        }
    }

    Ok(CollectionImportPreview {
        source_path: path.to_path_buf(),
        manifest,
        matched,
        ambiguous,
        unmatched,
    })
}

fn parse_collection_preset_toml(
    content: &str,
    path: &Path,
) -> Result<CollectionPresetManifest, CollectionExchangeError> {
    let manifest: CollectionPresetManifest =
        toml::from_str(content).map_err(|e| CollectionExchangeError::Toml {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;

    if manifest.schema_version != COLLECTION_PRESET_SCHEMA_VERSION {
        return Err(CollectionExchangeError::UnsupportedSchemaVersion {
            version: manifest.schema_version.clone(),
            supported: COLLECTION_PRESET_SCHEMA_VERSION.to_string(),
        });
    }

    if manifest.name.trim().is_empty() {
        return Err(CollectionExchangeError::InvalidManifest {
            message: "collection preset must include a non-empty name".to_string(),
        });
    }

    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::SyncSource;
    use tempfile::tempdir;

    fn sample_profile_named(name: &str, steam_app_id: &str) -> GameProfile {
        let mut p = GameProfile::default();
        p.game.name = name.to_string();
        p.steam.app_id = steam_app_id.to_string();
        p
    }

    fn register_profile(
        metadata: &MetadataStore,
        store: &ProfileStore,
        filename: &str,
        profile: &GameProfile,
    ) {
        store.save(filename, profile).unwrap();
        metadata
            .observe_profile_write(
                filename,
                profile,
                &PathBuf::from("/profiles").join(format!("{filename}.toml")),
                SyncSource::AppWrite,
                None,
            )
            .unwrap();
    }

    #[test]
    fn parse_rejects_future_schema_version() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("bad.toml");
        fs::write(
            &path,
            r#"
schema_version = "2"
name = "X"
profiles = []
"#,
        )
        .unwrap();

        let store = ProfileStore::with_base_path(dir.path().join("profiles"));
        let err = preview_collection_preset_import(&store, &path).unwrap_err();
        assert!(matches!(
            err,
            CollectionExchangeError::UnsupportedSchemaVersion { .. }
        ));
    }

    #[test]
    fn parse_rejects_malformed_toml() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("bad.toml");
        fs::write(&path, "not toml [[[").unwrap();
        let store = ProfileStore::with_base_path(dir.path().join("profiles"));
        let err = preview_collection_preset_import(&store, &path).unwrap_err();
        assert!(matches!(err, CollectionExchangeError::Toml { .. }));
    }

    #[test]
    fn export_preview_roundtrip_with_effective_app_id() {
        let dir = tempdir().unwrap();
        let profiles_dir = dir.path().join("profiles");
        let store = ProfileStore::with_base_path(profiles_dir.clone());
        let metadata = MetadataStore::open_in_memory().unwrap();

        let mut p = GameProfile::default();
        p.game.name = "Elden Ring".to_string();
        p.steam.app_id = String::new();
        p.runtime.steam_app_id = "1245620".to_string();
        p.trainer.community_trainer_sha256 = "ab".repeat(32);

        register_profile(&metadata, &store, "elden-ring", &p);

        let cid = metadata.create_collection("Action").unwrap();
        metadata
            .add_profile_to_collection(&cid, "elden-ring")
            .unwrap();

        let mut defaults = CollectionDefaultsSection::default();
        defaults.method = Some("proton_run".to_string());
        metadata
            .set_collection_defaults(&cid, Some(&defaults))
            .unwrap();

        let out = dir.path().join("out.crosshook-collection.toml");
        export_collection_preset_to_toml(&metadata, &store, &cid, &out).unwrap();

        let preview = preview_collection_preset_import(&store, &out).unwrap();
        assert_eq!(preview.manifest.name, "Action");
        assert_eq!(preview.matched.len(), 1);
        assert_eq!(preview.matched[0].local_profile_name, "elden-ring");
        assert!(preview.ambiguous.is_empty());
        assert!(preview.unmatched.is_empty());
        assert_eq!(
            preview.manifest.defaults.as_ref().and_then(|d| d.method.as_deref()),
            Some("proton_run")
        );
    }

    #[test]
    fn steam_match_ambiguous() {
        let dir = tempdir().unwrap();
        let store = ProfileStore::with_base_path(dir.path().join("profiles"));
        let metadata = MetadataStore::open_in_memory().unwrap();

        let a = sample_profile_named("A", "1");
        let b = sample_profile_named("B", "1");
        register_profile(&metadata, &store, "pa", &a);
        register_profile(&metadata, &store, "pb", &b);

        let manifest = CollectionPresetManifest {
            schema_version: COLLECTION_PRESET_SCHEMA_VERSION.to_string(),
            name: "Import".to_string(),
            description: None,
            defaults: None,
            profiles: vec![CollectionPresetProfileDescriptor {
                steam_app_id: "1".to_string(),
                game_name: String::new(),
                trainer_community_trainer_sha256: String::new(),
            }],
        };

        let path = dir.path().join("p.toml");
        write_preset_toml(&path, &manifest).unwrap();
        let preview = preview_collection_preset_import(&store, &path).unwrap();
        assert_eq!(preview.ambiguous.len(), 1);
        assert_eq!(preview.ambiguous[0].candidates.len(), 2);
    }

    #[test]
    fn pair_fallback_match() {
        let dir = tempdir().unwrap();
        let store = ProfileStore::with_base_path(dir.path().join("profiles"));
        let metadata = MetadataStore::open_in_memory().unwrap();

        let mut p = GameProfile::default();
        p.game.name = "Game".to_string();
        p.steam.app_id = String::new();
        p.trainer.community_trainer_sha256 = "cd".repeat(32);

        register_profile(&metadata, &store, "g1", &p);

        let manifest = CollectionPresetManifest {
            schema_version: COLLECTION_PRESET_SCHEMA_VERSION.to_string(),
            name: "X".to_string(),
            description: None,
            defaults: None,
            profiles: vec![CollectionPresetProfileDescriptor {
                steam_app_id: String::new(),
                game_name: "Game".to_string(),
                trainer_community_trainer_sha256: "cd".repeat(32),
            }],
        };

        let path = dir.path().join("p.toml");
        write_preset_toml(&path, &manifest).unwrap();
        let preview = preview_collection_preset_import(&store, &path).unwrap();
        assert_eq!(preview.matched.len(), 1);
        assert_eq!(preview.matched[0].local_profile_name, "g1");
    }

    #[test]
    fn unmatched_descriptor() {
        let dir = tempdir().unwrap();
        let store = ProfileStore::with_base_path(dir.path().join("profiles"));
        let _metadata = MetadataStore::open_in_memory().unwrap();

        let manifest = CollectionPresetManifest {
            schema_version: COLLECTION_PRESET_SCHEMA_VERSION.to_string(),
            name: "X".to_string(),
            description: None,
            defaults: None,
            profiles: vec![CollectionPresetProfileDescriptor {
                steam_app_id: "999".to_string(),
                game_name: "Nope".to_string(),
                trainer_community_trainer_sha256: String::new(),
            }],
        };

        let path = dir.path().join("p.toml");
        write_preset_toml(&path, &manifest).unwrap();
        let preview = preview_collection_preset_import(&store, &path).unwrap();
        assert_eq!(preview.unmatched.len(), 1);
    }

    #[test]
    fn export_fails_when_member_profile_missing() {
        let dir = tempdir().unwrap();
        let profiles_dir = dir.path().join("profiles");
        let store = ProfileStore::with_base_path(profiles_dir.clone());
        let metadata = MetadataStore::open_in_memory().unwrap();

        let p = sample_profile_named("X", "1");
        register_profile(&metadata, &store, "gone", &p);

        let cid = metadata.create_collection("C").unwrap();
        metadata
            .add_profile_to_collection(&cid, "gone")
            .unwrap();

        fs::remove_file(profiles_dir.join("gone.toml")).unwrap();

        let out = dir.path().join("out.toml");
        let err = export_collection_preset_to_toml(&metadata, &store, &cid, &out).unwrap_err();
        assert!(matches!(
            err,
            CollectionExchangeError::InvalidManifest { .. }
                | CollectionExchangeError::ProfileStore { .. }
        ));
    }
}
