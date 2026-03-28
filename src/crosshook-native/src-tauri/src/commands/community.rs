use crosshook_core::community::{
    CommunityProfileIndex, CommunityTapStore, CommunityTapSubscription, CommunityTapSyncResult,
};
use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::{import_community_profile, CommunityImportResult, ProfileStore};
use crosshook_core::settings::{AppSettingsData, SettingsStore};
use tauri::State;

fn map_error(error: impl ToString) -> String {
    error.to_string()
}

fn dedupe_taps(taps: Vec<CommunityTapSubscription>) -> Vec<CommunityTapSubscription> {
    let mut unique = Vec::new();

    for tap in taps {
        let already_present = unique.iter().any(|existing: &CommunityTapSubscription| {
            existing.url == tap.url && existing.branch == tap.branch
        });

        if !already_present {
            unique.push(tap);
        }
    }

    unique
}

fn load_settings(store: &SettingsStore) -> Result<AppSettingsData, String> {
    store.load().map_err(map_error)
}

fn load_community_taps(
    settings_store: &SettingsStore,
) -> Result<Vec<CommunityTapSubscription>, String> {
    Ok(load_settings(settings_store)?.community_taps)
}

fn save_community_taps(
    settings_store: &SettingsStore,
    mut settings: AppSettingsData,
    taps: Vec<CommunityTapSubscription>,
) -> Result<Vec<CommunityTapSubscription>, String> {
    let deduped = dedupe_taps(taps);
    settings.community_taps = deduped.clone();
    settings_store.save(&settings).map_err(map_error)?;
    Ok(deduped)
}

fn current_workspaces(
    tap_store: &CommunityTapStore,
    taps: &[CommunityTapSubscription],
) -> Result<Vec<crosshook_core::community::CommunityTapWorkspace>, String> {
    taps.iter()
        .map(|tap| tap_store.resolve_workspace(tap).map_err(map_error))
        .collect()
}

#[tauri::command]
pub fn community_add_tap(
    tap: CommunityTapSubscription,
    settings_store: State<'_, SettingsStore>,
) -> Result<Vec<CommunityTapSubscription>, String> {
    let settings = load_settings(&settings_store)?;
    let mut taps = settings.community_taps.clone();
    taps.push(tap);
    save_community_taps(&settings_store, settings, taps)
}

#[tauri::command]
pub fn community_list_profiles(
    settings_store: State<'_, SettingsStore>,
    tap_store: State<'_, CommunityTapStore>,
) -> Result<CommunityProfileIndex, String> {
    let taps = load_community_taps(&settings_store)?;
    let workspaces = current_workspaces(&tap_store, &taps)?;
    tap_store.index_workspaces(&workspaces).map_err(map_error)
}

#[tauri::command]
pub fn community_import_profile(
    path: String,
    profile_store: State<'_, ProfileStore>,
    settings_store: State<'_, SettingsStore>,
    tap_store: State<'_, CommunityTapStore>,
) -> Result<CommunityImportResult, String> {
    let import_path = std::path::Path::new(&path);
    validate_import_path_in_workspace(import_path, &settings_store, &tap_store)?;
    import_community_profile(import_path, &profile_store.base_path).map_err(map_error)
}

fn validate_import_path_in_workspace(
    path: &std::path::Path,
    settings_store: &SettingsStore,
    tap_store: &CommunityTapStore,
) -> Result<(), String> {
    let taps = load_community_taps(settings_store)?;
    let workspaces = current_workspaces(tap_store, &taps)?;

    let canonical = path.canonicalize().map_err(|error| {
        format!(
            "cannot resolve community profile path '{}': {error}",
            path.display()
        )
    })?;

    let in_workspace = workspaces.iter().any(|workspace| {
        workspace
            .local_path
            .canonicalize()
            .map(|root| canonical.starts_with(&root))
            .unwrap_or(false)
    });

    if !in_workspace {
        return Err(format!(
            "community profile path '{}' is not inside a known tap workspace",
            path.display()
        ));
    }

    Ok(())
}

#[tauri::command]
pub fn community_sync(
    settings_store: State<'_, SettingsStore>,
    tap_store: State<'_, CommunityTapStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<Vec<CommunityTapSyncResult>, String> {
    let taps = load_community_taps(&settings_store)?;
    let results = tap_store.sync_many(&taps).map_err(map_error)?;

    for result in &results {
        if let Err(e) = metadata_store.index_community_tap_result(result) {
            tracing::warn!(%e, tap_url = %result.workspace.subscription.url,
                "community tap index sync failed");
        }
    }

    Ok(results)
}

#[tauri::command]
pub fn community_list_indexed_profiles(
    metadata_store: State<'_, MetadataStore>,
) -> Result<Vec<crosshook_core::metadata::CommunityProfileRow>, String> {
    metadata_store.list_community_tap_profiles(None).map_err(map_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        let _ = community_add_tap
            as fn(
                CommunityTapSubscription,
                State<'_, SettingsStore>,
            ) -> Result<Vec<CommunityTapSubscription>, String>;
        let _ = community_list_profiles
            as fn(
                State<'_, SettingsStore>,
                State<'_, CommunityTapStore>,
            ) -> Result<CommunityProfileIndex, String>;
        let _ = community_import_profile
            as fn(
                String,
                State<'_, ProfileStore>,
                State<'_, SettingsStore>,
                State<'_, CommunityTapStore>,
            ) -> Result<CommunityImportResult, String>;
        let _ = community_sync
            as fn(
                State<'_, SettingsStore>,
                State<'_, CommunityTapStore>,
                State<'_, MetadataStore>,
            ) -> Result<Vec<CommunityTapSyncResult>, String>;
        let _ = community_list_indexed_profiles
            as fn(
                State<'_, MetadataStore>,
            ) -> Result<Vec<crosshook_core::metadata::CommunityProfileRow>, String>;
    }

    #[test]
    fn dedupes_taps_by_url_and_branch() {
        let taps = dedupe_taps(vec![
            CommunityTapSubscription {
                url: "https://example.invalid/community.git".to_string(),
                branch: Some("main".to_string()),
            },
            CommunityTapSubscription {
                url: "https://example.invalid/community.git".to_string(),
                branch: Some("main".to_string()),
            },
            CommunityTapSubscription {
                url: "https://example.invalid/community.git".to_string(),
                branch: Some("beta".to_string()),
            },
        ]);

        assert_eq!(taps.len(), 2);
    }
}
