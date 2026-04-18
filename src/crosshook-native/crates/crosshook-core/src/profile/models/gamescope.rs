use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GamescopeFilter {
    Fsr,
    Nis,
    Linear,
    Nearest,
    Pixel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GamescopeConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub internal_width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub internal_height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_rate_limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fsr_sharpness: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upscale_filter: Option<GamescopeFilter>,
    #[serde(default)]
    pub fullscreen: bool,
    #[serde(default)]
    pub borderless: bool,
    #[serde(default)]
    pub grab_cursor: bool,
    #[serde(default)]
    pub force_grab_cursor: bool,
    #[serde(default)]
    pub hdr_enabled: bool,
    /// When true, launch gamescope even inside an existing gamescope session.
    #[serde(default)]
    pub allow_nested: bool,
    /// Extra CLI arguments passed verbatim to gamescope.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_args: Vec<String>,
}

impl GamescopeConfig {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}
