use crosshook_core::launch::catalog::global_catalog;
use crosshook_core::metadata::{ConfigRevisionSource, MetadataStore, SyncSource};
use crosshook_core::profile::ProfileStore;
use crosshook_core::protondb::{
    lookup_protondb, AcceptSuggestionRequest, AcceptSuggestionResult, ProtonDbLookupResult,
    ProtonDbSuggestionSet, derive_suggestions, validate_env_suggestion,
};
use tauri::State;

use super::profile::capture_config_revision;

#[tauri::command]
pub async fn protondb_lookup(
    app_id: String,
    force_refresh: Option<bool>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<ProtonDbLookupResult, String> {
    let metadata_store = metadata_store.inner().clone();
    Ok(lookup_protondb(&metadata_store, &app_id, force_refresh.unwrap_or(false)).await)
}

#[tauri::command]
pub async fn protondb_get_suggestions(
    app_id: String,
    profile_name: String,
    force_refresh: Option<bool>,
    metadata_store: State<'_, MetadataStore>,
    profile_store: State<'_, ProfileStore>,
) -> Result<ProtonDbSuggestionSet, String> {
    let metadata_store_inner = metadata_store.inner().clone();

    let lookup_result =
        lookup_protondb(&metadata_store_inner, &app_id, force_refresh.unwrap_or(false)).await;

    let profile = profile_store
        .load(&profile_name)
        .map_err(|e| e.to_string())?;

    let catalog = global_catalog();

    let dismissed_keys = if metadata_store.is_available() {
        let profile_id = metadata_store
            .lookup_profile_id(&profile_name)
            .map_err(|e| e.to_string())?;
        if let Some(pid) = profile_id {
            metadata_store
                .get_dismissed_keys(&pid, &app_id)
                .map_err(|e| e.to_string())?
        } else {
            std::collections::HashSet::new()
        }
    } else {
        std::collections::HashSet::new()
    };

    Ok(derive_suggestions(
        &lookup_result,
        &profile,
        &catalog.entries,
        &dismissed_keys,
    ))
}

#[tauri::command]
pub fn protondb_accept_suggestion(
    request: AcceptSuggestionRequest,
    profile_store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<AcceptSuggestionResult, String> {
    match request {
        AcceptSuggestionRequest::Catalog {
            profile_name,
            catalog_entry_id,
        } => {
            let profile_name = profile_name.trim().to_string();
            if profile_name.is_empty() {
                return Err("profile_name is required".to_string());
            }

            let entry_id = catalog_entry_id.trim().to_string();
            if entry_id.is_empty() {
                return Err("catalog_entry_id is required".to_string());
            }

            let catalog = global_catalog();
            let entry = catalog
                .find_by_id(&entry_id)
                .ok_or_else(|| format!("unknown catalog entry: {entry_id}"))?;

            // Validate all env keys in the catalog entry.
            for pair in &entry.env {
                let key = pair[0].as_str();
                let value = pair[1].as_str();
                if !validate_env_suggestion(key, value) {
                    return Err(format!(
                        "catalog entry '{entry_id}' contains unsafe env pair: {key}={value}"
                    ));
                }
            }

            let mut profile = profile_store
                .load(&profile_name)
                .map_err(|e| e.to_string())?;

            let already_present = profile
                .launch
                .optimizations
                .enabled_option_ids
                .contains(&entry_id);

            if !already_present {
                profile
                    .launch
                    .optimizations
                    .enabled_option_ids
                    .push(entry_id.clone());
            }

            profile_store
                .save(&profile_name, &profile)
                .map_err(|e| e.to_string())?;

            let profile_path = profile_store
                .base_path
                .join(format!("{profile_name}.toml"));
            if let Err(e) = metadata_store.observe_profile_write(
                &profile_name,
                &profile,
                &profile_path,
                SyncSource::AppWrite,
                None,
            ) {
                tracing::warn!(
                    %e,
                    profile_name = %profile_name,
                    "metadata sync after protondb_accept_suggestion (catalog) failed"
                );
            }

            capture_config_revision(
                &profile_name,
                &profile,
                ConfigRevisionSource::ProtonDbSuggestionApply,
                None,
                &metadata_store,
            );

            let applied_keys: Vec<String> = entry.env.iter().map(|p| p[0].clone()).collect();
            let toggled_option_ids = if already_present {
                Vec::new()
            } else {
                vec![entry_id]
            };

            Ok(AcceptSuggestionResult {
                updated_profile: profile,
                applied_keys,
                toggled_option_ids,
            })
        }

        AcceptSuggestionRequest::EnvVar {
            profile_name,
            env_key,
            env_value,
        } => {
            let profile_name = profile_name.trim().to_string();
            if profile_name.is_empty() {
                return Err("profile_name is required".to_string());
            }

            // Re-validate at write time — never trust cached suggestion data.
            if !validate_env_suggestion(&env_key, &env_value) {
                return Err(format!(
                    "env var suggestion failed safety validation: {env_key}={env_value}"
                ));
            }

            let mut profile = profile_store
                .load(&profile_name)
                .map_err(|e| e.to_string())?;

            profile
                .launch
                .custom_env_vars
                .insert(env_key.clone(), env_value);

            profile_store
                .save(&profile_name, &profile)
                .map_err(|e| e.to_string())?;

            let profile_path = profile_store
                .base_path
                .join(format!("{profile_name}.toml"));
            if let Err(e) = metadata_store.observe_profile_write(
                &profile_name,
                &profile,
                &profile_path,
                SyncSource::AppWrite,
                None,
            ) {
                tracing::warn!(
                    %e,
                    profile_name = %profile_name,
                    "metadata sync after protondb_accept_suggestion (env_var) failed"
                );
            }

            capture_config_revision(
                &profile_name,
                &profile,
                ConfigRevisionSource::ProtonDbSuggestionApply,
                None,
                &metadata_store,
            );

            Ok(AcceptSuggestionResult {
                updated_profile: profile,
                applied_keys: vec![env_key],
                toggled_option_ids: Vec::new(),
            })
        }
    }
}

#[tauri::command]
pub fn protondb_dismiss_suggestion(
    profile_name: String,
    app_id: String,
    suggestion_key: String,
    metadata_store: State<'_, MetadataStore>,
    profile_store: State<'_, ProfileStore>,
) -> Result<(), String> {
    let profile_name = profile_name.trim();
    if profile_name.is_empty() {
        return Err("profile_name is required".to_string());
    }

    // Load the profile to resolve the profile_id via the metadata store.
    // We don't need the profile itself — just confirm it exists and get the id.
    let _ = profile_store
        .load(profile_name)
        .map_err(|e| e.to_string())?;

    let profile_id = metadata_store
        .lookup_profile_id(profile_name)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| {
            format!("profile '{profile_name}' has no metadata record — cannot dismiss suggestion")
        })?;

    metadata_store
        .dismiss_suggestion(&profile_id, &app_id, &suggestion_key, 30)
        .map_err(|e| e.to_string())?;

    Ok(())
}
