use std::path::PathBuf;

use crate::launch::runtime_helpers::resolve_wine_prefix_path;
use crate::profile::ProfileStore;

use super::types::{PrefixReference, ProfilePrefixReferences};
use super::utils::normalized_path_string;

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
        let configured = effective.runtime.prefix_path.trim();
        if configured.is_empty() {
            continue;
        }
        let configured_path = PathBuf::from(configured);
        let resolved_path = resolve_wine_prefix_path(&configured_path);
        references.push(PrefixReference {
            profile_name: name,
            configured_prefix_path: configured_path.to_string_lossy().into_owned(),
            resolved_prefix_path: normalized_path_string(&resolved_path),
        });
    }

    Ok(ProfilePrefixReferences {
        references,
        profiles_load_failed,
    })
}
