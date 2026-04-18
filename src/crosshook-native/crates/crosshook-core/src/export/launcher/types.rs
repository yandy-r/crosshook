use std::env;
use std::error::Error;
use std::fmt;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::paths::normalize_host_unix_path;
use crate::profile::{GamescopeConfig, TrainerLoadingMode};
use crate::settings::UmuPreference;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SteamExternalLauncherExportRequest {
    pub method: String,
    pub launcher_name: String,
    pub trainer_path: String,
    #[serde(default)]
    pub trainer_loading_mode: TrainerLoadingMode,
    pub launcher_icon_path: String,
    pub prefix_path: String,
    pub proton_path: String,
    pub steam_app_id: String,
    pub steam_client_install_path: String,
    pub target_home_path: String,
    #[serde(default)]
    pub profile_name: Option<String>,
    #[serde(default)]
    pub runtime_steam_app_id: String,
    #[serde(default)]
    pub umu_game_id: String,
    #[serde(default)]
    pub umu_preference: UmuPreference,
    #[serde(default)]
    pub network_isolation: bool,
    #[serde(default)]
    pub gamescope: GamescopeConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SteamExternalLauncherExportResult {
    pub display_name: String,
    pub launcher_slug: String,
    pub script_path: String,
    pub desktop_entry_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SteamExternalLauncherExportValidationError {
    TrainerPathRequired,
    PrefixPathRequired,
    ProtonPathRequired,
    SteamAppIdRequired,
    TargetHomePathRequired,
    LauncherIconPathNotFound,
    LauncherIconPathInvalidExtension,
    UnsupportedMethod(String),
}

impl SteamExternalLauncherExportValidationError {
    pub fn message(&self) -> &'static str {
        match self {
            Self::TrainerPathRequired => "External launcher export requires a trainer path.",
            Self::PrefixPathRequired => "External launcher export requires a prefix path.",
            Self::ProtonPathRequired => "External launcher export requires a Proton path.",
            Self::SteamAppIdRequired => "External launcher export requires a Steam App ID.",
            Self::TargetHomePathRequired => "External launcher export requires a host home path.",
            Self::LauncherIconPathNotFound => "External launcher export icon path does not exist.",
            Self::LauncherIconPathInvalidExtension => {
                "External launcher export icon must be a PNG or JPG image."
            }
            Self::UnsupportedMethod(_) => {
                "External launcher export only supports steam_applaunch and proton_run."
            }
        }
    }
}

impl fmt::Display for SteamExternalLauncherExportValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedMethod(method) => write!(
                f,
                "External launcher export only supports steam_applaunch and proton_run, not '{method}'."
            ),
            _ => f.write_str(self.message()),
        }
    }
}

impl Error for SteamExternalLauncherExportValidationError {}

#[derive(Debug)]
pub enum SteamExternalLauncherExportError {
    InvalidRequest(SteamExternalLauncherExportValidationError),
    CouldNotResolveHomePath,
    Io(io::Error),
}

impl fmt::Display for SteamExternalLauncherExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest(error) => error.fmt(f),
            Self::CouldNotResolveHomePath => {
                f.write_str("Could not resolve a host home path for launcher export.")
            }
            Self::Io(error) => write!(f, "{error}"),
        }
    }
}

impl Error for SteamExternalLauncherExportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for SteamExternalLauncherExportError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

pub fn validate(
    request: &SteamExternalLauncherExportRequest,
) -> Result<(), SteamExternalLauncherExportValidationError> {
    match request.method.trim() {
        "steam_applaunch" | "proton_run" => {}
        other => {
            return Err(
                SteamExternalLauncherExportValidationError::UnsupportedMethod(other.to_string()),
            )
        }
    }

    if request.trainer_path.trim().is_empty() {
        return Err(SteamExternalLauncherExportValidationError::TrainerPathRequired);
    }

    if request.prefix_path.trim().is_empty() {
        return Err(SteamExternalLauncherExportValidationError::PrefixPathRequired);
    }

    if request.proton_path.trim().is_empty() {
        return Err(SteamExternalLauncherExportValidationError::ProtonPathRequired);
    }

    if request.target_home_path.trim().is_empty()
        && env::var("HOME").unwrap_or_default().trim().is_empty()
    {
        return Err(SteamExternalLauncherExportValidationError::TargetHomePathRequired);
    }

    if request.method.trim() == "steam_applaunch" && request.steam_app_id.trim().is_empty() {
        return Err(SteamExternalLauncherExportValidationError::SteamAppIdRequired);
    }

    if !request.launcher_icon_path.trim().is_empty() {
        let normalized_icon_path = normalize_host_unix_path(&request.launcher_icon_path);
        let icon_path = Path::new(&normalized_icon_path);

        if !icon_path.exists() {
            return Err(SteamExternalLauncherExportValidationError::LauncherIconPathNotFound);
        }

        let extension = icon_path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default();

        if !matches!(
            extension.to_ascii_lowercase().as_str(),
            "png" | "jpg" | "jpeg"
        ) {
            return Err(
                SteamExternalLauncherExportValidationError::LauncherIconPathInvalidExtension,
            );
        }
    }

    Ok(())
}
