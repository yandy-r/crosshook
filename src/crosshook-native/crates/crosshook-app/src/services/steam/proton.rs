use crosshook_core::steam::{discover_compat_tools, discover_steam_root_candidates, ProtonInstall};

use super::client::default_client_install_path_with_env;
use super::SteamService;
use crate::services::run_blocking;
use crate::AppError;

impl SteamService {
    /// Performs fresh Proton discovery on the blocking pool on every call.
    pub async fn list_proton_installs(
        &self,
        steam_client_install_path: Option<String>,
    ) -> Result<Vec<ProtonInstall>, AppError> {
        run_blocking(move || {
            let configured_path = steam_client_install_path.unwrap_or_else(|| {
                default_client_install_path_with_env(|key| std::env::var_os(key))
            });
            let mut diagnostics = Vec::new();
            let roots = discover_steam_root_candidates(configured_path, &mut diagnostics);
            let installs = discover_compat_tools(&roots, &mut diagnostics);
            for entry in &diagnostics {
                tracing::debug!(entry, "proton discovery diagnostic");
            }
            Ok(installs)
        })
        .await
    }
}
