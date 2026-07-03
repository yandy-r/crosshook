//! Pure trainer-coexistence advisory analysis over the registered mod set.
//! Rules only ever emit Warning/Info — advisories can never block a launch.

use std::path::Path;

use crate::launch::request::ValidationSeverity;
use crate::profile::{InjectionMethod, TrainerLoadingMode};

use super::detection::PROXY_DLL_NAMES;
use super::model::{ModCategory, ProfileModRecord};

/// Launch-shape-independent inputs so the analyzer is pure and exhaustively testable.
#[derive(Debug, Clone)]
pub struct CoexistenceContext {
    pub trainer_configured: bool,
    pub trainer_loading_mode: TrainerLoadingMode,
    pub injection_method: InjectionMethod,
    /// File names (basenames, lowercased) from the effective profile's injection.dll_paths.
    pub injection_dll_names: Vec<String>,
    pub mangohud_enabled: bool,
    pub gamescope_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoexistenceAdvisory {
    pub code: &'static str,
    /// Warning | Info ONLY — rules cannot construct Fatal.
    pub severity: ValidationSeverity,
    pub message: String,
    pub help: String,
}

pub(super) struct CoexistenceRule {
    pub(super) code: &'static str,
    pub(super) severity: ValidationSeverity,
    pub(super) applies: fn(&ProfileModRecord, &CoexistenceContext) -> bool,
    pub(super) message: fn(&ProfileModRecord, &CoexistenceContext) -> String,
    pub(super) help: &'static str,
}

pub(super) const RULES: &[CoexistenceRule] = &[
    CoexistenceRule {
        code: "mod_coexistence_injection_vector",
        severity: ValidationSeverity::Warning,
        applies: |m, ctx| shares_injection_vector(m, ctx) && injection_dll_overlap(m, ctx),
        message: |m, _| {
            format!(
                "Mod “{}” and the configured trainer rely on the same in-process load vector.",
                m.name
            )
        },
        help: "Proxy-DLL loaders and script extenders hook the game process the same way CrossHook's injection/copy-to-prefix trainer paths do. If the game crashes at startup, disable one side or switch the trainer loading mode to source_directory.",
    },
    CoexistenceRule {
        code: "mod_coexistence_injection_vector",
        severity: ValidationSeverity::Info,
        applies: |m, ctx| {
            shares_injection_vector(m, ctx)
                && !injection_dll_overlap(m, ctx)
                && ctx.trainer_loading_mode == TrainerLoadingMode::CopyToPrefix
        },
        message: |m, _| {
            format!(
                "Mod “{}” may share the injection vector with the copy-to-prefix trainer loading mode.",
                m.name
            )
        },
        help: "Proxy-DLL loaders and script extenders hook the game process the same way CrossHook's injection/copy-to-prefix trainer paths do. If the game crashes at startup, disable one side or switch the trainer loading mode to source_directory.",
    },
    CoexistenceRule {
        code: "mod_coexistence_overlay_conflict",
        severity: ValidationSeverity::Warning,
        applies: |m, ctx| {
            m.category == ModCategory::OverlayInjection
                && (ctx.mangohud_enabled || ctx.gamescope_enabled)
        },
        message: |m, ctx| {
            format!(
                "Overlay mod “{}” is stacked with {}.",
                m.name,
                active_overlays(ctx)
            )
        },
        help: "Multiple present-hook overlays are a known crash/flicker source. If you see flicker or a black screen, disable MangoHud/gamescope for this profile or disable the overlay mod.",
    },
    CoexistenceRule {
        code: "mod_coexistence_file_replacement_notice",
        severity: ValidationSeverity::Info,
        applies: |m, _| m.category == ModCategory::FileReplacement,
        message: |m, _| format!("File-replacement mod “{}” is registered for this game.", m.name),
        help: "Hash baselines and detection scans may reflect modded files instead of vanilla ones. Launch is not blocked.",
    },
    CoexistenceRule {
        code: "mod_coexistence_script_extender_launch_order",
        severity: ValidationSeverity::Info,
        applies: |m, ctx| m.category == ModCategory::ScriptExtender && ctx.trainer_configured,
        message: |m, _| {
            format!(
                "Script extender “{}” launches the real game executable itself.",
                m.name
            )
        },
        help: "Launch the game through the extender's loader and attach the trainer after the real game process is up; attach/launch order matters.",
    },
];

/// Runs the rules table over the enabled mod set. Callers pass only enabled
/// mods, but disabled records are filtered defensively as well.
pub fn analyze_mod_coexistence(
    enabled_mods: &[ProfileModRecord],
    ctx: &CoexistenceContext,
) -> Vec<CoexistenceAdvisory> {
    if enabled_mods.is_empty() {
        return Vec::new();
    }

    let mut advisories = Vec::new();
    for rule in RULES {
        for record in enabled_mods.iter().filter(|record| record.enabled) {
            if (rule.applies)(record, ctx) {
                advisories.push(CoexistenceAdvisory {
                    code: rule.code,
                    severity: rule.severity,
                    message: (rule.message)(record, ctx),
                    help: rule.help.to_string(),
                });
            }
        }
    }
    advisories
}

/// A trainer is configured and the mod hooks the process the way CrossHook's
/// trainer paths do (script extender, or overlay shipping a proxy-loader DLL).
fn shares_injection_vector(record: &ProfileModRecord, ctx: &CoexistenceContext) -> bool {
    if !ctx.trainer_configured {
        return false;
    }
    record.category == ModCategory::ScriptExtender
        || (record.category == ModCategory::OverlayInjection
            && path_basenames_lower(record)
                .iter()
                .any(|name| PROXY_DLL_NAMES.contains(&name.as_str())))
}

/// The mod ships a DLL whose basename collides with a configured injection DLL.
fn injection_dll_overlap(record: &ProfileModRecord, ctx: &CoexistenceContext) -> bool {
    ctx.injection_method != InjectionMethod::Disabled
        && path_basenames_lower(record)
            .iter()
            .any(|name| ctx.injection_dll_names.iter().any(|dll| dll == name))
}

fn path_basenames_lower(record: &ProfileModRecord) -> Vec<String> {
    record
        .paths
        .iter()
        .filter_map(|path| Path::new(path).file_name().and_then(|name| name.to_str()))
        .map(str::to_ascii_lowercase)
        .collect()
}

fn active_overlays(ctx: &CoexistenceContext) -> &'static str {
    match (ctx.mangohud_enabled, ctx.gamescope_enabled) {
        (true, true) => "MangoHud and gamescope",
        (true, false) => "MangoHud",
        _ => "gamescope",
    }
}
