use crosshook_core::metadata::MetadataStore;
use crosshook_core::offline::{
    global_trainer_type_catalog, is_network_available, HashVerifyResult, OfflineReadinessReport,
    TrainerTypeEntry,
};
use crosshook_core::profile::ProfileStore;
use tauri::State;

#[tauri::command]
pub async fn check_offline_readiness(
    name: String,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<OfflineReadinessReport, String> {
    if !metadata_store.is_available() {
        return Err("metadata store unavailable".to_string());
    }
    let store = store.inner().clone();
    let metadata_store = metadata_store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let profile = store.load(&name).map_err(|e| e.to_string())?;
        let profile_id = metadata_store
            .lookup_profile_id(&name)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("profile id not found for {name}"))?;
        metadata_store
            .check_offline_readiness_for_profile(&name, &profile_id, &profile)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn batch_offline_readiness(
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<Vec<OfflineReadinessReport>, String> {
    if !metadata_store.is_available() {
        return Err("metadata store unavailable".to_string());
    }
    let store = store.inner().clone();
    let metadata_store = metadata_store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let names = store.list().map_err(|e| e.to_string())?;
        let mut out = Vec::with_capacity(names.len());
        for name in names {
            let profile = match store.load(&name) {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!(profile = %name, error = %e, "skip profile in batch_offline_readiness");
                    continue;
                }
            };
            let pid = match metadata_store.lookup_profile_id(&name) {
                Ok(Some(id)) => id,
                Ok(None) => {
                    tracing::warn!(profile = %name, "skip profile without profile_id");
                    continue;
                }
                Err(e) => {
                    tracing::warn!(profile = %name, error = %e, "skip profile in batch_offline_readiness (lookup failed)");
                    continue;
                }
            };
            let report = match metadata_store
                .check_offline_readiness_for_profile(&name, &pid, &profile)
            {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(profile = %name, error = %e, "skip profile in batch_offline_readiness (readiness check failed)");
                    continue;
                }
            };
            out.push(report);
        }
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn verify_trainer_hash(
    name: String,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<HashVerifyResult, String> {
    if !metadata_store.is_available() {
        return Err("metadata store unavailable".to_string());
    }
    let store = store.inner().clone();
    let metadata_store = metadata_store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let profile = store.load(&name).map_err(|e| e.to_string())?;
        let profile_id = metadata_store
            .lookup_profile_id(&name)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("profile id not found for {name}"))?;
        let path = std::path::PathBuf::from(profile.effective_profile().trainer.path.trim());
        if path.as_os_str().is_empty() {
            return Err("trainer path is empty".to_string());
        }
        let res = metadata_store
            .verify_trainer_hash_for_profile_path(&profile_id, &path)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "trainer file missing or unreadable".to_string())?;
        Ok(res)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn check_network_status() -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(|| Ok(is_network_available()))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn get_trainer_type_catalog() -> Result<Vec<TrainerTypeEntry>, String> {
    Ok(global_trainer_type_catalog().entries().to_vec())
}
