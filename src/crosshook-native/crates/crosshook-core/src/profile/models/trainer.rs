use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TrainerLoadingMode {
    #[default]
    SourceDirectory,
    CopyToPrefix,
}

impl TrainerLoadingMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SourceDirectory => "source_directory",
            Self::CopyToPrefix => "copy_to_prefix",
        }
    }
}

impl FromStr for TrainerLoadingMode {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "source_directory" => Ok(Self::SourceDirectory),
            "copy_to_prefix" => Ok(Self::CopyToPrefix),
            _ => Err("unsupported trainer loading mode"),
        }
    }
}

pub(super) fn default_trainer_type() -> String {
    "unknown".to_string()
}

fn is_default_trainer_type(s: &String) -> bool {
    s == "unknown"
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainerSection {
    #[serde(default)]
    pub path: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(rename = "loading_mode", default)]
    pub loading_mode: TrainerLoadingMode,
    #[serde(
        default = "default_trainer_type",
        skip_serializing_if = "is_default_trainer_type"
    )]
    pub trainer_type: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_protontricks: Vec<String>,
    /// Optional SHA-256 from a community profile manifest (advisory comparison at launch).
    #[serde(
        rename = "community_trainer_sha256",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    pub community_trainer_sha256: String,
}

impl Default for TrainerSection {
    fn default() -> Self {
        Self {
            path: String::new(),
            kind: String::new(),
            loading_mode: TrainerLoadingMode::default(),
            trainer_type: default_trainer_type(),
            required_protontricks: Vec::new(),
            community_trainer_sha256: String::new(),
        }
    }
}
