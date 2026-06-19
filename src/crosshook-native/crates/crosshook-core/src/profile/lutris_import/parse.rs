use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::error::LutrisImportError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LutrisGameConfig {
    pub name: Option<String>,
    #[serde(rename = "game-slug")]
    pub game_slug: Option<String>,
    pub game: Option<LutrisGameSection>,
    pub system: Option<LutrisSystemSection>,
    pub wine: Option<LutrisWineSection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LutrisGameSection {
    pub exe: Option<String>,
    pub args: Option<String>,
    pub working_dir: Option<String>,
    pub prefix: Option<String>,
    pub arch: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LutrisSystemSection {
    pub env: Option<BTreeMap<String, String>>,
    pub terminal: Option<bool>,
    pub gamemode: Option<bool>,
    pub mangohud: Option<bool>,
    pub prefer_system_libs: Option<bool>,
    pub gamescope: Option<bool>,
    pub gamescope_hdr: Option<bool>,
    pub gamescope_force_grab_cursor: Option<bool>,
    pub gamescope_output_res: Option<String>,
    pub gamescope_game_res: Option<String>,
    pub gamescope_window_mode: Option<String>,
    pub gamescope_fsr_sharpness: Option<String>,
    pub gamescope_fps_limiter: Option<String>,
    pub gamescope_flags: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LutrisWineSection {
    pub version: Option<String>,
    pub dxvk: Option<bool>,
    pub vkd3d: Option<bool>,
    pub dxvk_nvapi: Option<bool>,
    pub esync: Option<bool>,
    pub fsync: Option<bool>,
}

pub fn parse_lutris_yaml(path: &Path) -> Result<LutrisGameConfig, LutrisImportError> {
    let contents = fs::read_to_string(path).map_err(|err| LutrisImportError::Io {
        action: "read".to_string(),
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;

    serde_yaml_ng::from_str(&contents).map_err(|err| LutrisImportError::Yaml {
        path: path.to_path_buf(),
        message: err.to_string(),
    })
}
