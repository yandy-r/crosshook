//! Pure dependency-tool selection shared by check and install (D13).

use crosshook_core::prefix_deps::validation::parse_nonzero_steam_app_id;
use crosshook_core::prefix_deps::{BinaryDetectionResult, PrefixDepsTool};
use crosshook_core::profile::GameProfile;

/// The tool a check or install runs with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedTool {
    /// Tool binary: an absolute path or a bare name resolved on PATH.
    pub binary_path: String,
    /// Tool kind (decides the argv shape).
    pub tool: PrefixDepsTool,
    /// Canonical Steam App ID; always `None` for winetricks.
    pub steam_app_id: Option<String>,
}

/// Why no tool could be selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolSelectionError {
    /// Neither winetricks nor protontricks was found.
    NotFound,
    /// Only protontricks is available and the profile has no valid Steam App ID.
    ProtontricksNeedsAppId,
}

/// Selects the tool for `detection`.
///
/// Protontricks needs a valid nonzero Steam App ID (`steam_app_id` is parsed
/// again, so a raw value is fine); without one, winetricks on PATH is used
/// instead. `winetricks_on_path` runs only for that fallback.
///
/// # Errors
/// [`ToolSelectionError::NotFound`] when detection found nothing;
/// [`ToolSelectionError::ProtontricksNeedsAppId`] when protontricks lacks an
/// App ID and winetricks is not on PATH.
pub fn resolve_tool(
    detection: &BinaryDetectionResult,
    steam_app_id: Option<&str>,
    winetricks_on_path: impl FnOnce() -> Option<String>,
) -> Result<ResolvedTool, ToolSelectionError> {
    let binary_path = match (&detection.binary_path, detection.found) {
        (Some(path), true) if !path.is_empty() => path.clone(),
        _ => return Err(ToolSelectionError::NotFound),
    };
    let tool = detection.tool_type.unwrap_or_else(|| {
        if detection.binary_name.contains("protontricks") {
            PrefixDepsTool::Protontricks
        } else {
            PrefixDepsTool::Winetricks
        }
    });
    match tool {
        PrefixDepsTool::Winetricks => Ok(ResolvedTool {
            binary_path,
            tool,
            steam_app_id: None,
        }),
        PrefixDepsTool::Protontricks => {
            if let Some(app_id) = steam_app_id.and_then(parse_nonzero_steam_app_id) {
                return Ok(ResolvedTool {
                    binary_path,
                    tool,
                    steam_app_id: Some(app_id),
                });
            }
            winetricks_on_path()
                .map(|winetricks| ResolvedTool {
                    binary_path: winetricks,
                    tool: PrefixDepsTool::Winetricks,
                    steam_app_id: None,
                })
                .ok_or(ToolSelectionError::ProtontricksNeedsAppId)
        }
    }
}

/// The profile's Steam App ID: `steam.app_id`, else `runtime.steam_app_id`,
/// each only when it is a valid nonzero `u32`.
pub fn profile_steam_app_id(effective: &GameProfile) -> Option<String> {
    parse_nonzero_steam_app_id(&effective.steam.app_id)
        .or_else(|| parse_nonzero_steam_app_id(&effective.runtime.steam_app_id))
}
