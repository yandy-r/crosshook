use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SteamLibrary {
    #[serde(default)]
    pub path: PathBuf,
    #[serde(default)]
    pub steamapps_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SteamGameMatch {
    #[serde(default)]
    pub app_id: String,
    #[serde(default)]
    pub library_path: PathBuf,
    #[serde(default)]
    pub install_dir_path: PathBuf,
    #[serde(default)]
    pub manifest_path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SteamAutoPopulateFieldState {
    #[default]
    NotFound,
    Found,
    Ambiguous,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SteamAutoPopulateResult {
    #[serde(default)]
    pub app_id_state: SteamAutoPopulateFieldState,
    #[serde(default)]
    pub app_id: String,
    #[serde(default)]
    pub compatdata_state: SteamAutoPopulateFieldState,
    #[serde(default)]
    pub compatdata_path: PathBuf,
    #[serde(default)]
    pub proton_state: SteamAutoPopulateFieldState,
    #[serde(default)]
    pub proton_path: PathBuf,
    #[serde(default)]
    pub diagnostics: Vec<String>,
    #[serde(default)]
    pub manual_hints: Vec<String>,
}

impl SteamAutoPopulateResult {
    pub fn has_any_match(&self) -> bool {
        matches!(self.app_id_state, SteamAutoPopulateFieldState::Found)
            || matches!(self.compatdata_state, SteamAutoPopulateFieldState::Found)
            || matches!(self.proton_state, SteamAutoPopulateFieldState::Found)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SteamAutoPopulateRequest {
    #[serde(default)]
    pub game_path: PathBuf,
    #[serde(default)]
    pub steam_client_install_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProtonInstall {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub path: PathBuf,
    #[serde(default)]
    pub is_official: bool,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub normalized_aliases: BTreeSet<String>,
}
