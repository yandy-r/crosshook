//! Local profile match index and descriptor classification used by import-preview.

use std::collections::HashMap;

use crate::profile::collection_schema::CollectionPresetProfileDescriptor;
use crate::profile::{resolve_art_app_id, ProfileStore};

use super::error::CollectionExchangeError;
use super::types::CollectionPresetMatchCandidate;

pub(super) struct LocalMatchIndex {
    pub(super) steam_to_profiles: HashMap<String, Vec<String>>,
    pub(super) pair_to_profiles: HashMap<(String, String), Vec<String>>,
    /// Display info for ambiguous candidates.
    pub(super) profile_display: HashMap<String, (String, String)>,
}

pub(super) enum MatchClass {
    Matched { profile: String },
    Ambiguous { names: Vec<String> },
    Unmatched,
}

pub(super) fn build_local_match_index(
    store: &ProfileStore,
) -> Result<LocalMatchIndex, CollectionExchangeError> {
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

        profile_display.insert(profile_name.clone(), (game_name.clone(), app_id.clone()));

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

pub(super) fn candidates_for_names(
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

pub(super) fn classify_descriptor(
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
