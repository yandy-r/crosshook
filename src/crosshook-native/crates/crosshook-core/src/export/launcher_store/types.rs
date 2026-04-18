//! Public types for the launcher store: result structs and the error type.

use std::error::Error;
use std::fmt;
use std::io;

use serde::{Deserialize, Serialize};

/// Metadata about an exported launcher pair on disk.
///
/// `is_stale` is only meaningful when the value was derived with profile context
/// via `check_launcher_exists` / `check_launcher_for_profile`. `list_launchers`
/// does not have that context and currently reports `is_stale = false`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LauncherInfo {
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub launcher_slug: String,
    #[serde(default)]
    pub script_path: String,
    #[serde(default)]
    pub desktop_entry_path: String,
    #[serde(default)]
    pub script_exists: bool,
    #[serde(default)]
    pub desktop_entry_exists: bool,
    #[serde(default)]
    pub is_stale: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LauncherDeleteResult {
    #[serde(default)]
    pub script_deleted: bool,
    #[serde(default)]
    pub desktop_entry_deleted: bool,
    #[serde(default)]
    pub script_path: String,
    #[serde(default)]
    pub desktop_entry_path: String,
    #[serde(default)]
    pub script_skipped_reason: Option<String>,
    #[serde(default)]
    pub desktop_entry_skipped_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LauncherRenameResult {
    #[serde(default)]
    pub old_slug: String,
    #[serde(default)]
    pub new_slug: String,
    #[serde(default)]
    pub new_script_path: String,
    #[serde(default)]
    pub new_desktop_entry_path: String,
    #[serde(default)]
    pub script_renamed: bool,
    #[serde(default)]
    pub desktop_entry_renamed: bool,
    #[serde(default)]
    pub old_script_cleanup_warning: Option<String>,
    #[serde(default)]
    pub old_desktop_entry_cleanup_warning: Option<String>,
}

#[derive(Debug)]
pub enum LauncherStoreError {
    Io(io::Error),
    HomePathResolutionFailed,
}

impl fmt::Display for LauncherStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::HomePathResolutionFailed => {
                f.write_str("Could not resolve a host home path for launcher operations.")
            }
        }
    }
}

impl Error for LauncherStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for LauncherStoreError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}
