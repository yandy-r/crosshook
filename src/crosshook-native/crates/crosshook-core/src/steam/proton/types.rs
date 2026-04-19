use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

use super::super::models::SteamAutoPopulateFieldState;

pub(super) const SYSTEM_COMPAT_TOOL_ROOTS: &[&str] = &[
    "/usr/share/steam/compatibilitytools.d",
    "/usr/local/share/steam/compatibilitytools.d",
    "/usr/share/steam/compatibilitytools",
    "/usr/local/share/steam/compatibilitytools",
];

pub type CompatToolMappings = HashMap<String, BTreeSet<String>>;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProtonResolution {
    pub state: SteamAutoPopulateFieldState,
    pub proton_path: PathBuf,
}
