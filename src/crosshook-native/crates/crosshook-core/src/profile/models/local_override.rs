use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LocalOverrideSection {
    #[serde(default)]
    pub game: LocalOverrideGameSection,
    #[serde(default)]
    pub trainer: LocalOverrideTrainerSection,
    #[serde(default)]
    pub steam: LocalOverrideSteamSection,
    #[serde(default)]
    pub runtime: LocalOverrideRuntimeSection,
}

impl LocalOverrideSection {
    pub fn is_empty(&self) -> bool {
        self.game.is_empty()
            && self.trainer.is_empty()
            && self.steam.is_empty()
            && self.runtime.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LocalOverrideGameSection {
    #[serde(rename = "executable_path", default)]
    pub executable_path: String,
    #[serde(rename = "custom_cover_art_path", default)]
    pub custom_cover_art_path: String,
    #[serde(rename = "custom_portrait_art_path", default)]
    pub custom_portrait_art_path: String,
    #[serde(rename = "custom_background_art_path", default)]
    pub custom_background_art_path: String,
}

impl LocalOverrideGameSection {
    pub fn is_empty(&self) -> bool {
        self.executable_path.trim().is_empty()
            && self.custom_cover_art_path.trim().is_empty()
            && self.custom_portrait_art_path.trim().is_empty()
            && self.custom_background_art_path.trim().is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LocalOverrideTrainerSection {
    #[serde(default)]
    pub path: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_protontricks: Vec<String>,
}

impl LocalOverrideTrainerSection {
    pub fn is_empty(&self) -> bool {
        self.path.trim().is_empty() && self.extra_protontricks.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LocalOverrideSteamSection {
    #[serde(rename = "compatdata_path", default)]
    pub compatdata_path: String,
    #[serde(rename = "proton_path", default)]
    pub proton_path: String,
}

impl LocalOverrideSteamSection {
    pub fn is_empty(&self) -> bool {
        self.compatdata_path.trim().is_empty() && self.proton_path.trim().is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LocalOverrideRuntimeSection {
    #[serde(rename = "prefix_path", default)]
    pub prefix_path: String,
    #[serde(rename = "proton_path", default)]
    pub proton_path: String,
}

impl LocalOverrideRuntimeSection {
    pub fn is_empty(&self) -> bool {
        self.prefix_path.trim().is_empty() && self.proton_path.trim().is_empty()
    }
}
