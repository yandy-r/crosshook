use std::path::PathBuf;

use crosshook_core::profile::ProfileStore;
use crosshook_core::settings::SettingsStore;

use crate::cli_error::CliError;

pub(crate) fn profile_store(profile_dir: Option<PathBuf>) -> Result<ProfileStore, CliError> {
    match profile_dir {
        Some(path) => Ok(ProfileStore::with_base_path(path)),
        None => {
            let settings_store = SettingsStore::try_new()
                .map_err(|e| CliError::General(format!("settings store: {e}")))?;
            let settings = settings_store
                .load()
                .map_err(|e| CliError::General(format!("settings load: {e}")))?;
            ProfileStore::try_new_with_settings_data(&settings, &settings_store.base_path)
                .map_err(|e| CliError::General(format!("failed to initialize profile store: {e}")))
        }
    }
}
