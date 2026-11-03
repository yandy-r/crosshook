use std::path::Path;

use crate::launch::runtime_helpers::resolve_wine_prefix_path;
use crate::profile::{GameProfile, ProfileStore};

use super::types::{PrefixReference, PrefixReferenceSource, ProfilePrefixReferences};
use super::utils::normalized_path_string;

#[cfg(test)]
mod tests;

pub fn collect_profile_prefix_references(
    store: &ProfileStore,
) -> Result<ProfilePrefixReferences, String> {
    let names = store
        .list()
        .map_err(|error| format!("failed to list profiles for prefix scan: {error}"))?;

    let mut references = Vec::new();
    let mut profiles_load_failed = false;
    for name in names {
        let profile = match store.load(&name) {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!(profile = %name, %error, "skipping profile during prefix scan");
                profiles_load_failed = true;
                continue;
            }
        };
        let effective = profile.effective_profile();
        references.extend(profile_prefix_references(&name, &effective));
    }

    Ok(ProfilePrefixReferences {
        references,
        profiles_load_failed,
    })
}

/// Every prefix a profile names, tagged with the field it came from.
///
/// Both `runtime.prefix_path` and `steam.compatdata_path` are reported regardless
/// of launch method (`steam_applaunch` runs in the compat-data prefix, other
/// methods in the runtime prefix), so a prefix that any profile names is never
/// treated as orphaned. When both fields resolve to the same prefix (community
/// imports copy `compatdata_path` into `prefix_path`), it is reported once as a
/// Steam compat-data reference so the Steam-owned prefix stays protect-only.
fn profile_prefix_references(name: &str, profile: &GameProfile) -> Vec<PrefixReference> {
    let reference = |configured: &str, source| PrefixReference {
        profile_name: name.to_string(),
        configured_prefix_path: configured.to_string(),
        resolved_prefix_path: normalized_path_string(&resolve_wine_prefix_path(Path::new(
            configured,
        ))),
        source,
    };
    let runtime = profile.runtime.prefix_path.trim();
    let compatdata = profile.steam.compatdata_path.trim();
    let compatdata_reference = (!compatdata.is_empty())
        .then(|| reference(compatdata, PrefixReferenceSource::SteamCompatdataPath));
    let runtime_reference = (!runtime.is_empty())
        .then(|| reference(runtime, PrefixReferenceSource::RuntimePrefixPath))
        .filter(|runtime_reference| {
            compatdata_reference
                .as_ref()
                .is_none_or(|compatdata_reference| {
                    compatdata_reference.resolved_prefix_path
                        != runtime_reference.resolved_prefix_path
                })
        });
    runtime_reference
        .into_iter()
        .chain(compatdata_reference)
        .collect()
}
