use crosshook_core::settings::{
    AppSettingsData, RecentFilesData, RecentFilesStore, RecentFilesStoreError, SettingsStore,
    SettingsStoreError,
};
use tauri::State;

fn map_settings_error(error: SettingsStoreError) -> String {
    error.to_string()
}

fn map_recent_files_error(error: RecentFilesStoreError) -> String {
    error.to_string()
}

#[tauri::command]
pub fn settings_load(store: State<'_, SettingsStore>) -> Result<AppSettingsData, String> {
    store.load().map_err(map_settings_error)
}

#[tauri::command]
pub fn settings_save(data: AppSettingsData, store: State<'_, SettingsStore>) -> Result<(), String> {
    store.save(&data).map_err(map_settings_error)
}

#[tauri::command]
pub fn recent_files_load(store: State<'_, RecentFilesStore>) -> Result<RecentFilesData, String> {
    store.load().map_err(map_recent_files_error)
}

#[tauri::command]
pub fn recent_files_save(
    data: RecentFilesData,
    store: State<'_, RecentFilesStore>,
) -> Result<(), String> {
    store.save(&data).map_err(map_recent_files_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        let _ = settings_load as fn(State<'_, SettingsStore>) -> Result<AppSettingsData, String>;
        let _ =
            settings_save as fn(AppSettingsData, State<'_, SettingsStore>) -> Result<(), String>;
        let _ =
            recent_files_load as fn(State<'_, RecentFilesStore>) -> Result<RecentFilesData, String>;
        let _ = recent_files_save
            as fn(RecentFilesData, State<'_, RecentFilesStore>) -> Result<(), String>;
    }
}
