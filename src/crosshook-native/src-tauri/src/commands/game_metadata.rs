use crosshook_core::game_images::{
    download_and_cache_image, import_custom_art as core_import_art,
    import_custom_cover_art as core_import_cover_art, GameImageType,
};
use crosshook_core::metadata::MetadataStore;
use crosshook_core::settings::SettingsStore;
use crosshook_core::steam_metadata::{lookup_steam_metadata, SteamMetadataLookupResult};
use tauri::State;

#[tauri::command]
pub async fn fetch_game_metadata(
    app_id: String,
    force_refresh: Option<bool>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<SteamMetadataLookupResult, String> {
    let store = metadata_store.inner().clone();
    Ok(lookup_steam_metadata(&store, &app_id, force_refresh.unwrap_or(false)).await)
}

#[tauri::command]
pub async fn fetch_game_cover_art(
    app_id: String,
    image_type: Option<String>,
    metadata_store: State<'_, MetadataStore>,
    settings_store: State<'_, SettingsStore>,
) -> Result<Option<String>, String> {
    let store = metadata_store.inner().clone();
    let image_type = match image_type.as_deref().unwrap_or("cover") {
        "cover" => GameImageType::Cover,
        "hero" => GameImageType::Hero,
        "capsule" => GameImageType::Capsule,
        "portrait" => GameImageType::Portrait,
        "background" => GameImageType::Background,
        other => {
            tracing::warn!("Unknown image_type requested: {other}");
            return Err(format!("Unknown image type: {other}"));
        }
    };

    // Read the SteamGridDB API key from settings (non-fatal on error).
    let api_key_owned: Option<String> = settings_store
        .load()
        .ok()
        .and_then(|s| s.steamgriddb_api_key)
        .filter(|k| !k.trim().is_empty());
    let api_key = api_key_owned.as_deref();

    download_and_cache_image(&store, &app_id, image_type, api_key).await
}

#[tauri::command]
pub fn import_custom_cover_art(source_path: String) -> Result<String, String> {
    core_import_cover_art(&source_path)
}

#[tauri::command]
pub fn import_custom_art(source_path: String, art_type: Option<String>) -> Result<String, String> {
    let image_type = match art_type.as_deref().unwrap_or("cover") {
        "cover" => GameImageType::Cover,
        "portrait" => GameImageType::Portrait,
        "background" => GameImageType::Background,
        other => return Err(format!("Unknown art type: {other}")),
    };
    core_import_art(&source_path, image_type)
}
