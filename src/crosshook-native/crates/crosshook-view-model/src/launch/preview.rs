//! Plain-data launch inputs the pipeline derivations read: the subset of the
//! core `LaunchPreview` (dry run) they use, and the profile fields of the
//! config-only pipeline.
//!
//! Field names match the core and IPC names, so the `app` feature converts
//! core values one to one and the parity fixtures deserialize unchanged.

use super::LaunchMethod;

/// Profile fields the config-only (tier 1) pipeline checks; every path is
/// "configured" when its trimmed value is non-empty.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub struct PipelineProfile {
    /// `game.executable_path`.
    pub game_executable_path: String,
    /// `runtime.prefix_path`.
    pub prefix_path: String,
    /// `runtime.proton_path`.
    pub proton_path: String,
    /// `steam.app_id`.
    pub steam_app_id: String,
    /// `trainer.path`.
    pub trainer_path: String,
    /// Number of `launch.optimizations.enabled_option_ids`.
    pub enabled_optimizations: usize,
}

/// Severity of a [`ValidationIssue`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(test, derive(serde::Deserialize), serde(rename_all = "snake_case"))]
pub enum Severity {
    /// Blocks the launch.
    Fatal,
    /// Shown, does not block.
    Warning,
    /// Informational.
    Info,
}

/// One launch validation issue.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub struct ValidationIssue {
    /// User-facing message.
    pub message: String,
    /// Severity.
    pub severity: Severity,
    /// Machine-readable kind (`ValidationError::code()` in core), if any.
    pub code: Option<String>,
}

/// Validation summary of a preview.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub struct PreviewValidation {
    /// Every issue, in backend order.
    pub issues: Vec<ValidationIssue>,
}

/// Where a preview environment variable comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(test, derive(serde::Deserialize), serde(rename_all = "snake_case"))]
pub enum EnvVarSource {
    /// Core Proton runtime variables.
    ProtonRuntime,
    /// Launch optimization toggles.
    LaunchOptimization,
    /// Host passthrough.
    Host,
    /// Steam-specific Proton variables.
    SteamProton,
    /// Profile custom variables.
    ProfileCustom,
}

impl EnvVarSource {
    /// Readable label (`envSourceLabel`).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ProtonRuntime => "Proton runtime",
            Self::LaunchOptimization => "Launch optimization",
            Self::Host => "Host",
            Self::SteamProton => "Steam + Proton",
            Self::ProfileCustom => "Profile custom",
        }
    }
}

/// One environment variable the launch sets.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub struct EnvVar {
    /// Variable name.
    pub key: String,
    /// Value.
    pub value: String,
    /// Source.
    pub source: EnvVarSource,
}

/// Why a wrapper appears in the launch chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(test, derive(serde::Deserialize), serde(rename_all = "snake_case"))]
pub enum WrapperSource {
    /// A launch optimization.
    Optimization,
    /// Network isolation (`unshare --net`).
    NetworkIsolation,
    /// Gamescope.
    Gamescope,
}

/// One wrapper of the launch chain, in invocation order.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub struct WrapperDetail {
    /// Command tokens.
    pub command: Vec<String>,
    /// Source.
    pub source: WrapperSource,
    /// False when configured but not run.
    pub active: bool,
    /// Human-readable reason.
    pub reason: String,
    /// Invocation that absorbs this wrapper (e.g. `gamescope --mangoapp`).
    pub folded_into: Option<String>,
}

/// Proton setup of a non-native preview.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub struct ProtonSetup {
    /// Wine prefix path.
    pub wine_prefix_path: String,
    /// Proton executable path.
    pub proton_executable: String,
}

/// Trainer section of a preview.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub struct PreviewTrainer {
    /// Trainer path.
    pub path: String,
}

/// umu-database coverage of the profile's app id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(test, derive(serde::Deserialize), serde(rename_all = "snake_case"))]
pub enum CsvCoverage {
    /// The app id has a row.
    Found,
    /// The CSV is readable but has no row for the app id.
    Missing,
    /// No CSV source is reachable.
    Unknown,
}

impl CsvCoverage {
    /// Wire name (`found`, `missing`, `unknown`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Found => "found",
            Self::Missing => "missing",
            Self::Unknown => "unknown",
        }
    }
}

/// How the umu GAMEID was resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(test, derive(serde::Deserialize), serde(rename_all = "snake_case"))]
#[allow(missing_docs)] // Variants mirror core `UmuGameIdResolutionSource`; see `label`.
pub enum GameIdSource {
    ExplicitOverride,
    SteamAppId,
    FreshCache,
    FreshLookup,
    StaleCache,
    CachedNotFound,
    LookupDisabled,
    MissingHints,
    ApiUnavailable,
    Fallback,
}

impl GameIdSource {
    /// Readable label (`umuGameIdResolutionSourceLabel`).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ExplicitOverride => "profile override",
            Self::SteamAppId => "Steam app id",
            Self::FreshCache => "cache hit",
            Self::FreshLookup => "fresh HTTP lookup",
            Self::StaleCache => "stale cache fallback",
            Self::CachedNotFound => "cached not found",
            Self::LookupDisabled => "lookup disabled",
            Self::MissingHints => "missing store/codename",
            Self::ApiUnavailable => "API unavailable",
            Self::Fallback => "fallback",
        }
    }
}

/// Resolved umu GAMEID.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub struct GameIdResolution {
    /// The GAMEID.
    pub game_id: String,
    /// How it was resolved.
    pub source: GameIdSource,
}

/// Why the launch will (or will not) use `umu-run`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub struct UmuDecision {
    /// `true`: `umu-run`; `false`: direct Proton.
    pub will_use_umu: bool,
    /// Human-readable reason.
    pub reason: String,
    /// umu-database coverage.
    pub csv_coverage: CsvCoverage,
    /// GAMEID resolution, if any.
    pub gameid_resolution: Option<GameIdResolution>,
}

/// The dry-run preview fields the pipeline and its detail panel read.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub struct LaunchPreview {
    /// Method after inference.
    pub resolved_method: LaunchMethod,
    /// Validation issues.
    pub validation: PreviewValidation,
    /// Environment; `None` when collection failed.
    pub environment: Option<Vec<EnvVar>>,
    /// Variables cleared before launch.
    #[cfg_attr(test, serde(default))]
    pub cleared_variables: Vec<String>,
    /// Wrapper tokens; `None` when directive resolution failed.
    pub wrappers: Option<Vec<String>>,
    /// Wrapper attribution; `None` when directive resolution failed.
    pub wrapper_details: Option<Vec<WrapperDetail>>,
    /// Effective command; `None` when directive resolution failed.
    pub effective_command: Option<String>,
    /// Directive resolution or command building error.
    pub directives_error: Option<String>,
    /// Steam launch options (`steam_applaunch` only).
    pub steam_launch_options: Option<String>,
    /// Proton setup (non-native methods).
    pub proton_setup: Option<ProtonSetup>,
    /// Full game executable path.
    #[cfg_attr(test, serde(default))]
    pub game_executable: String,
    /// Game executable file name.
    #[cfg_attr(test, serde(default))]
    pub game_executable_name: String,
    /// Trainer section, if a trainer is configured.
    pub trainer: Option<PreviewTrainer>,
    /// umu decision (`proton_run` only).
    pub umu_decision: Option<UmuDecision>,
}

/// Last segment of a `/`- or `\`-separated path, after trimming
/// (`lastPathSegment`); empty for a blank path.
#[must_use]
pub fn last_path_segment(path: &str) -> &str {
    let trimmed = path.trim();
    trimmed.rsplit(['/', '\\']).next().unwrap_or(trimmed)
}

/// Trimmed value when non-empty.
pub(crate) fn non_blank(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|v| !v.is_empty())
}
