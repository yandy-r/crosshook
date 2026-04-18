use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MangoHudPosition {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    TopCenter,
    BottomCenter,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MangoHudConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fps_limit: Option<u32>,
    #[serde(default)]
    pub gpu_stats: bool,
    #[serde(default)]
    pub cpu_stats: bool,
    #[serde(default)]
    pub ram: bool,
    #[serde(default)]
    pub frametime: bool,
    #[serde(default)]
    pub battery: bool,
    #[serde(default)]
    pub watt: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<MangoHudPosition>,
}

impl MangoHudConfig {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}
