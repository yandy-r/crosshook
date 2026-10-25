use crosshook_app::ProfileWriter;
use tauri::State;

#[tauri::command]
pub async fn profile_set_favorite(
    name: String,
    favorite: bool,
    writer: State<'_, ProfileWriter>,
) -> Result<(), String> {
    writer
        .set_favorite(name, favorite)
        .await
        .map(|commit| commit.into_output())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn profile_list_favorites(
    writer: State<'_, ProfileWriter>,
) -> Result<Vec<String>, String> {
    writer
        .list_favorites()
        .await
        .map_err(|error| error.to_string())
}
