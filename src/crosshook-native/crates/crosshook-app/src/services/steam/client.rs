use std::ffi::OsString;
use std::path::PathBuf;

use crosshook_core::steam::{
    attempt_auto_populate, SteamAutoPopulateRequest, SteamAutoPopulateResult,
};

use super::SteamService;
use crate::services::run_blocking;
use crate::AppError;

impl SteamService {
    /// Resolves the default client root without blocking the caller's executor.
    pub async fn default_client_install_path(&self) -> String {
        run_blocking(|| {
            Ok(default_client_install_path_with_env(|key| {
                std::env::var_os(key)
            }))
        })
        .await
        .unwrap_or_default()
    }

    /// Discovers Steam fields for the supplied game without modifying a profile.
    pub async fn auto_populate(
        &self,
        request: SteamAutoPopulateRequest,
    ) -> Result<SteamAutoPopulateResult, AppError> {
        run_blocking(move || Ok(attempt_auto_populate(&request))).await
    }
}

// A free resolver keeps tests independent of process-global environment state.
// The override deliberately is not checked for existence or canonicalized.
pub(super) fn default_client_install_path_with_env(
    mut lookup: impl FnMut(&str) -> Option<OsString>,
) -> String {
    if let Some(value) = lookup("STEAM_COMPAT_CLIENT_INSTALL_PATH") {
        if let Some(value) = value.to_str() {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }

    let Some(home) = lookup("HOME").map(PathBuf::from) else {
        return String::new();
    };
    for candidate in [
        home.join(".local/share/Steam"),
        home.join(".steam/root"),
        home.join(".var/app/com.valvesoftware.Steam/data/Steam"),
    ] {
        if candidate.join("steamapps").is_dir() {
            return candidate.to_string_lossy().into_owned();
        }
    }
    String::new()
}
