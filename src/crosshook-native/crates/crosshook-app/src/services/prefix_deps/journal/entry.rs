//! Validation of a persisted journal row before it is replayed (I7, I9, M4,
//! S-6).

use std::path::Path;

use crosshook_core::metadata::PrefixVersionRestoreJournalRow;
use crosshook_core::platform::normalized_path_is_executable_file_on_host;
use crosshook_core::prefix_deps::runner::{is_windows_version_verb, normalize_prefix_path};
use crosshook_core::prefix_deps::validation::{
    parse_nonzero_steam_app_id, validate_protontricks_verbs,
};
use crosshook_core::prefix_deps::{
    PrefixDepsTool, PrefixVersionRestorePlan, PrefixVersionRestoreState,
};

pub(super) const STATE_PENDING: &str = "pending";
pub(super) const STATE_FAILED: &str = "failed";

/// Persisted row state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PendingOrFailed {
    Pending,
    Failed,
}

impl PendingOrFailed {
    pub(super) fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            STATE_PENDING => Ok(Self::Pending),
            STATE_FAILED => Ok(Self::Failed),
            other => Err(format!("invalid repair state in journal: {other}")),
        }
    }

    pub(super) fn restore_state(self) -> PrefixVersionRestoreState {
        match self {
            Self::Pending => PrefixVersionRestoreState::Pending,
            Self::Failed => PrefixVersionRestoreState::Failed,
        }
    }
}

/// A journal row validated for execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JournalEntry {
    pub(crate) plan: PrefixVersionRestorePlan,
    pub(crate) state: PendingOrFailed,
    pub(crate) last_error: Option<String>,
}

pub(super) fn parse_tool(raw: &str) -> Result<PrefixDepsTool, String> {
    match raw {
        "winetricks" => Ok(PrefixDepsTool::Winetricks),
        "protontricks" => Ok(PrefixDepsTool::Protontricks),
        other => Err(format!(
            "invalid dependency tool in repair journal: {other}"
        )),
    }
}

pub(super) fn tool_name(tool: PrefixDepsTool) -> &'static str {
    match tool {
        PrefixDepsTool::Winetricks => "winetricks",
        PrefixDepsTool::Protontricks => "protontricks",
    }
}

/// M4: a replayed row runs only a bare tool name or an absolute executable.
fn validate_binary_path(binary_path: &str) -> Result<(), String> {
    if matches!(binary_path, "winetricks" | "protontricks") {
        return Ok(());
    }
    if Path::new(binary_path).is_absolute()
        && normalized_path_is_executable_file_on_host(binary_path)
    {
        return Ok(());
    }
    Err("repair journal has an invalid tool binary path".to_string())
}

/// S-6: a replayed row must name the prefix by the canonical `pfx` path the
/// caller keyed on, so the job key, the in-use probe and the restore all
/// target the same prefix. Blocking (canonicalizes).
pub(super) fn validate_prefix_path(canonical: &str, stored: &str) -> Result<(), String> {
    match normalize_prefix_path(stored) {
        Ok(normalized) if normalized == stored && stored == canonical => Ok(()),
        Ok(_) => Err("repair journal prefix path is not canonical".to_string()),
        Err(_) => Err("repair journal prefix path does not resolve to a prefix".to_string()),
    }
}

impl TryFrom<PrefixVersionRestoreJournalRow> for JournalEntry {
    type Error = String;

    /// Validates tool, state, restore verb (I9), the protontricks App ID and
    /// the binary path (M4); the error text never embeds the binary path.
    fn try_from(row: PrefixVersionRestoreJournalRow) -> Result<Self, Self::Error> {
        let tool_type = parse_tool(&row.tool_type)?;
        let state = PendingOrFailed::parse(&row.state)?;
        validate_protontricks_verbs(std::slice::from_ref(&row.restore_verb))
            .map_err(|error| error.to_string())?;
        if !is_windows_version_verb(&row.restore_verb) {
            return Err("repair journal has an invalid Windows version restore verb".to_string());
        }
        let steam_app_id = match tool_type {
            PrefixDepsTool::Protontricks => Some(
                row.steam_app_id
                    .as_deref()
                    .and_then(parse_nonzero_steam_app_id)
                    .ok_or_else(|| {
                        "repair journal has an invalid Steam App ID for protontricks; expected a positive decimal value"
                            .to_string()
                    })?,
            ),
            PrefixDepsTool::Winetricks => None,
        };
        validate_binary_path(&row.binary_path)?;
        Ok(Self {
            plan: PrefixVersionRestorePlan {
                binary_path: row.binary_path,
                prefix_path: row.prefix_path,
                tool_type,
                steam_app_id,
                restore_verb: row.restore_verb,
            },
            state,
            last_error: row.last_error,
        })
    }
}
