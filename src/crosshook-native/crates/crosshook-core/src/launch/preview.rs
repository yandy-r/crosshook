use std::path::Path;

use directories::BaseDirs;
use serde::Serialize;

use super::env::WINE_ENV_VARS_TO_CLEAR;
use super::optimizations::{
    build_steam_launch_options_command, resolve_launch_directives,
    resolve_launch_directives_for_method, LaunchDirectives,
};
use super::request::{
    is_inside_gamescope_session, validate_all, LaunchRequest, LaunchValidationIssue, METHOD_NATIVE,
    METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH,
};
use super::runtime_helpers::{
    build_gamescope_args, collect_pressure_vessel_paths, env_value, resolve_proton_paths,
    resolve_steam_client_install_path, DEFAULT_HOST_PATH,
};
use super::script_runner::{
    force_no_umu_for_launch_request, proton_path_dirname, resolve_launch_proton_path,
    should_use_umu,
};
use crate::profile::TrainerLoadingMode;

/// Source category for a preview environment variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvVarSource {
    /// Core Proton runtime vars (STEAM_COMPAT_DATA_PATH, WINEPREFIX, etc.)
    ProtonRuntime,
    /// Vars from launch optimization toggles (PROTON_NO_STEAMINPUT, etc.)
    LaunchOptimization,
    /// Passthrough from host (HOME, DISPLAY, PATH, etc.)
    Host,
    /// Steam-specific Proton vars for steam_applaunch
    SteamProton,
    /// Profile `launch.custom_env_vars` (wins over launch optimizations on key conflict)
    ProfileCustom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolvedLaunchMethod {
    SteamApplaunch,
    ProtonRun,
    Native,
}

impl ResolvedLaunchMethod {
    fn from_request(request: &LaunchRequest) -> Self {
        match request.resolved_method() {
            METHOD_STEAM_APPLAUNCH => Self::SteamApplaunch,
            METHOD_PROTON_RUN => Self::ProtonRun,
            METHOD_NATIVE => Self::Native,
            _ => Self::Native,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::SteamApplaunch => METHOD_STEAM_APPLAUNCH,
            Self::ProtonRun => METHOD_PROTON_RUN,
            Self::Native => METHOD_NATIVE,
        }
    }
}

/// A single environment variable that will be set during launch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PreviewEnvVar {
    pub key: String,
    pub value: String,
    pub source: EnvVarSource,
}

/// Proton runtime setup details (non-native methods only).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProtonSetup {
    pub wine_prefix_path: String,
    pub compat_data_path: String,
    pub steam_client_install_path: String,
    pub proton_executable: String,
    /// Path to `umu-run` if it will be used for this launch, otherwise `None`.
    pub umu_run_path: Option<String>,
}

/// Trainer configuration details for the preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PreviewTrainerInfo {
    pub path: String,
    pub host_path: String,
    pub loading_mode: TrainerLoadingMode,
    /// The Windows-side path when copy_to_prefix mode is used.
    pub staged_path: Option<String>,
}

/// Validation summary for the preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PreviewValidation {
    pub issues: Vec<LaunchValidationIssue>,
}

/// Complete dry-run preview result returned to the frontend.
///
/// Sections that depend on independent computations use `Option<T>` so
/// the preview can return partial results when one section fails (e.g.,
/// directive resolution fails but validation and game info are still useful).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LaunchPreview {
    /// The effective launch method after inference.
    pub resolved_method: ResolvedLaunchMethod,

    /// All validation results collected (not short-circuited).
    pub validation: PreviewValidation,

    /// Environment variables that will be set.
    /// None when environment collection fails (e.g., directive resolution error).
    pub environment: Option<Vec<PreviewEnvVar>>,

    /// WINE/Proton env vars that will be cleared before launch.
    pub cleared_variables: Vec<String>,

    /// Wrapper command chain (e.g. ["mangohud", "gamemoderun"]).
    /// None when directive resolution fails.
    pub wrappers: Option<Vec<String>>,

    /// Human-readable effective command string.
    /// None when directive resolution fails.
    pub effective_command: Option<String>,

    /// Error message when directive resolution or command building fails.
    /// Allows the frontend to show what went wrong alongside partial results.
    pub directives_error: Option<String>,

    /// Steam Launch Options %command% string (steam_applaunch only).
    pub steam_launch_options: Option<String>,

    /// Proton environment setup details.
    pub proton_setup: Option<ProtonSetup>,

    /// Resolved working directory.
    pub working_directory: String,

    /// Full game executable path.
    pub game_executable: String,

    /// Just the file name portion.
    pub game_executable_name: String,

    /// Trainer details (None for game-only or native launches).
    pub trainer: Option<PreviewTrainerInfo>,

    /// ISO 8601 timestamp when the preview was generated.
    pub generated_at: String,

    /// Pre-rendered display text for clipboard copy.
    pub display_text: String,

    /// Whether gamescope will be active for this launch.
    pub gamescope_active: bool,

    /// Diagnostic: how the umu decision was resolved for this preview.
    /// Only populated for `proton_run` method; `None` for `steam_applaunch` and `native`.
    pub umu_decision: Option<UmuDecisionPreview>,
}

/// Explains why the preview will (or will not) invoke `umu-run`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UmuDecisionPreview {
    /// The `umu_preference` value the backend received on the request.
    pub requested_preference: crate::settings::UmuPreference,
    /// `Some(path)` when `umu-run` was discovered on the backend's `PATH`.
    pub umu_run_path_on_backend_path: Option<String>,
    /// Final decision: `true` → emit `umu-run`, `false` → emit direct Proton.
    pub will_use_umu: bool,
    /// Human-readable reason the preview modal can surface.
    pub reason: String,
    /// Coverage status of the profile's app id in the umu-database CSV.
    /// `Unknown` when no app id is available or no CSV source is reachable.
    pub csv_coverage: crate::umu_database::CsvCoverage,
}

impl LaunchPreview {
    /// Renders a human-readable TOML-like text summary for clipboard copy.
    pub fn to_display_toml(&self) -> String {
        let mut lines = Vec::new();

        // [preview]
        lines.push("[preview]".to_string());
        lines.push(format!("generated_at = \"{}\"", self.generated_at));
        lines.push(format!("method = \"{}\"", self.resolved_method.as_str()));
        lines.push(format!("game = \"{}\"", self.game_executable));
        lines.push(format!("game_name = \"{}\"", self.game_executable_name));
        if !self.working_directory.is_empty() {
            lines.push(format!(
                "working_directory = \"{}\"",
                self.working_directory
            ));
        }
        lines.push(String::new());

        // [validation]
        lines.push("[validation]".to_string());
        lines.push(format!("passed = {}", self.validation.issues.is_empty()));
        lines.push(format!("issue_count = {}", self.validation.issues.len()));
        for issue in &self.validation.issues {
            lines.push(format!("  [{:?}] {}", issue.severity, issue.message));
        }
        lines.push(String::new());

        // [command]
        lines.push("[command]".to_string());
        if let Some(ref cmd) = self.effective_command {
            lines.push(format!("effective = \"{cmd}\""));
        }
        if let Some(ref opts) = self.steam_launch_options {
            lines.push(format!("steam_launch_options = \"{opts}\""));
        }
        if let Some(ref wrappers) = self.wrappers {
            if !wrappers.is_empty() {
                lines.push(format!("wrappers = {wrappers:?}"));
            }
        }
        if let Some(ref err) = self.directives_error {
            lines.push(format!("error = \"{err}\""));
        }
        lines.push(String::new());

        // [proton]
        if let Some(ref setup) = self.proton_setup {
            lines.push("[proton]".to_string());
            lines.push(format!(
                "proton_executable = \"{}\"",
                setup.proton_executable
            ));
            lines.push(format!("wine_prefix_path = \"{}\"", setup.wine_prefix_path));
            lines.push(format!("compat_data_path = \"{}\"", setup.compat_data_path));
            lines.push(format!(
                "steam_client_install_path = \"{}\"",
                setup.steam_client_install_path
            ));
            if let Some(ref umu) = setup.umu_run_path {
                lines.push(format!("umu_run = \"{umu}\""));
            }
            lines.push(String::new());
        }

        // [trainer]
        if let Some(ref trainer) = self.trainer {
            lines.push("[trainer]".to_string());
            lines.push(format!("path = \"{}\"", trainer.path));
            lines.push(format!("host_path = \"{}\"", trainer.host_path));
            lines.push(format!(
                "loading_mode = \"{}\"",
                trainer.loading_mode.as_str()
            ));
            if let Some(ref staged) = trainer.staged_path {
                lines.push(format!("staged_path = \"{staged}\""));
            }
            lines.push(String::new());
        }

        // [environment]
        if let Some(ref env) = self.environment {
            lines.push(format!("[environment]  # {} vars", env.len()));
            for var in env {
                lines.push(format!("{} = \"{}\"", var.key, var.value));
            }
            lines.push(String::new());
        }

        // [cleared_variables]
        if !self.cleared_variables.is_empty() {
            lines.push(format!(
                "[cleared_variables]  # {} vars",
                self.cleared_variables.len()
            ));
            for var in &self.cleared_variables {
                lines.push(format!("  {var}"));
            }
        }

        lines.join("\n")
    }
}

/// Builds a complete launch preview from a launch request.
///
/// Assembles validation, directives, environment, command, and metadata
/// into a single `LaunchPreview` struct for display in the frontend modal.
pub fn build_launch_preview(request: &LaunchRequest) -> Result<LaunchPreview, String> {
    let resolved_method = ResolvedLaunchMethod::from_request(request);
    let validation_issues = validate_all(request);
    let gamescope_config = request.effective_gamescope_config();

    let gamescope_active = gamescope_config.enabled
        && (gamescope_config.allow_nested || !is_inside_gamescope_session());

    // Resolve launch directives (wrappers + optimization env).
    // `steam_applaunch` uses the same optimization catalog as `proton_run` for Steam Launch Options,
    // without going through `resolve_launch_directives` (which is proton_run-only on the request).
    // Trainer-only launches use the same resolution path so previews match Proton-aligned semantics.
    // This can fail independently of validation (e.g., missing wrapper binary).
    let (directives, mut directives_error) = match request.resolved_method() {
        METHOD_STEAM_APPLAUNCH => match resolve_launch_directives_for_method(
            &request.optimizations.enabled_option_ids,
            METHOD_PROTON_RUN,
        ) {
            Ok(d) => (Some(d), None),
            Err(e) => (None, Some(e.to_string())),
        },
        _ => match resolve_launch_directives(request) {
            Ok(d) => (Some(d), None),
            Err(e) => (None, Some(e.to_string())),
        },
    };

    // Environment and command depend on successful directive resolution.
    let (environment, wrappers, effective_command) = match &directives {
        Some(directives) => {
            // Compute effective wrappers: prepend unshare for trainer-only + isolation.
            let effective_wrappers = if request.launch_trainer_only
                && request.network_isolation
                && super::runtime_helpers::is_unshare_net_available()
            {
                let mut w = vec!["unshare".to_string(), "--net".to_string()];
                w.extend(directives.wrappers.iter().cloned());
                w
            } else {
                directives.wrappers.clone()
            };

            let wrappers_had_mangohud = directives.wrappers.iter().any(|w| w.trim() == "mangohud");
            let mut env = Vec::new();
            collect_host_environment(&mut env);
            match resolved_method {
                ResolvedLaunchMethod::SteamApplaunch => {
                    collect_steam_proton_environment(request, &mut env);
                    merge_optimization_and_custom_preview_env(request, directives, &mut env);
                }
                ResolvedLaunchMethod::ProtonRun => {
                    collect_runtime_proton_environment(request, &mut env);
                    merge_optimization_and_custom_preview_env(request, directives, &mut env);
                }
                ResolvedLaunchMethod::Native => {
                    merge_custom_preview_env_only(request, &mut env);
                }
            }
            inject_mangohud_config_preview_env(
                &mut env,
                request,
                gamescope_active,
                wrappers_had_mangohud,
            );
            let effective_command = match build_effective_command_string(
                request,
                resolved_method,
                &effective_wrappers,
                &gamescope_config,
                gamescope_active,
            ) {
                Ok(command) => Some(command),
                Err(error) => {
                    append_preview_error(&mut directives_error, error);
                    None
                }
            };
            (Some(env), Some(effective_wrappers), effective_command)
        }
        None => (None, None, None),
    };

    // Steam launch options (for copy/paste); may still be computed when directive resolution failed
    // so errors surface consistently with the standalone Steam options panel.
    let gamescope_param = if gamescope_config.enabled {
        Some(gamescope_config)
    } else {
        None
    };
    let steam_launch_options = if resolved_method == ResolvedLaunchMethod::SteamApplaunch {
        match build_steam_launch_options_command(
            &request.optimizations.enabled_option_ids,
            &request.custom_env_vars,
            gamescope_param.as_ref(),
        ) {
            Ok(command) => Some(command),
            Err(error) => {
                append_preview_error(&mut directives_error, error.to_string());
                None
            }
        }
    } else {
        None
    };

    // These sections are independent of directive resolution.
    let proton_setup = build_proton_setup(request, resolved_method.as_str());
    let trainer = build_trainer_info(request, resolved_method.as_str());
    let cleared_variables = if resolved_method != ResolvedLaunchMethod::Native {
        WINE_ENV_VARS_TO_CLEAR
            .iter()
            .map(std::string::ToString::to_string)
            .collect()
    } else {
        Vec::new()
    };
    let working_directory = resolve_working_directory(request, resolved_method.as_str());
    let generated_at = chrono::Utc::now().to_rfc3339();

    let umu_decision = if resolved_method == ResolvedLaunchMethod::ProtonRun {
        Some(build_umu_decision_preview(request))
    } else {
        None
    };

    let mut preview = LaunchPreview {
        resolved_method,
        validation: PreviewValidation {
            issues: validation_issues,
        },
        environment,
        cleared_variables,
        wrappers,
        effective_command,
        directives_error,
        steam_launch_options,
        proton_setup,
        working_directory,
        game_executable: request.game_path.trim().to_string(),
        game_executable_name: request.game_executable_name(),
        trainer,
        generated_at,
        display_text: String::new(),
        gamescope_active,
        umu_decision,
    };

    preview.display_text = preview.to_display_toml();
    Ok(preview)
}

/// Builds the diagnostic `umu_decision` field for `proton_run` previews.
fn build_umu_decision_preview(request: &LaunchRequest) -> UmuDecisionPreview {
    use crate::settings::UmuPreference;
    let requested = request.umu_preference;
    let force_no_umu = force_no_umu_for_launch_request(request);
    let (will_use_umu, umu_run_path) = should_use_umu(request, force_no_umu);
    let reason = match (requested, umu_run_path.as_deref(), will_use_umu) {
        (_, _, true) => format!(
            "using umu-run at {}",
            umu_run_path.as_deref().unwrap_or("<unknown>")
        ),
        (UmuPreference::Proton, _, false) => {
            "preference = Proton — direct Proton always".to_string()
        }
        (UmuPreference::Auto, _, false) => {
            "preference = Auto but umu-run was not found on the backend PATH — falling back to direct Proton".to_string()
        }
        (UmuPreference::Umu, None, false) => {
            "preference = Umu but umu-run was not found on the backend PATH".to_string()
        }
        (UmuPreference::Umu, Some(_), false) => {
            "preference = Umu and umu-run found, but should_use_umu returned false (bug)"
                .to_string()
        }
    };
    let app_id = crate::launch::script_runner::resolve_steam_app_id_for_umu(request);
    let csv_coverage = crate::umu_database::check_coverage(app_id, Some("steam"));
    UmuDecisionPreview {
        requested_preference: requested,
        umu_run_path_on_backend_path: umu_run_path,
        will_use_umu,
        reason,
        csv_coverage,
    }
}

/// Collects host environment variables that will be passed through to the launch command.
fn collect_host_environment(env: &mut Vec<PreviewEnvVar>) {
    const DEFAULT_SHELL: &str = "/bin/bash";
    let host_vars: &[(&str, &str)] = &[
        ("HOME", ""),
        ("USER", ""),
        ("LOGNAME", ""),
        ("SHELL", DEFAULT_SHELL),
        ("PATH", DEFAULT_HOST_PATH),
        ("DISPLAY", ""),
        ("WAYLAND_DISPLAY", ""),
        ("XDG_RUNTIME_DIR", ""),
        ("DBUS_SESSION_BUS_ADDRESS", ""),
    ];

    for (key, default) in host_vars {
        env.push(PreviewEnvVar {
            key: key.to_string(),
            value: env_value(key, default),
            source: EnvVarSource::Host,
        });
    }
}

/// Collects Proton runtime environment variables for `proton_run` launches.
///
/// Uses `resolve_wine_prefix_path()` heuristic for WINEPREFIX resolution,
/// which differs from `steam_applaunch` (hardcoded `{compatdata}/pfx`).
fn collect_runtime_proton_environment(request: &LaunchRequest, env: &mut Vec<PreviewEnvVar>) {
    let resolved_paths = resolve_proton_paths(Path::new(request.runtime.prefix_path.trim()));

    env.push(PreviewEnvVar {
        key: "WINEPREFIX".to_string(),
        value: resolved_paths
            .wine_prefix_path
            .to_string_lossy()
            .into_owned(),
        source: EnvVarSource::ProtonRuntime,
    });

    env.push(PreviewEnvVar {
        key: "STEAM_COMPAT_DATA_PATH".to_string(),
        value: resolved_paths
            .compat_data_path
            .to_string_lossy()
            .into_owned(),
        source: EnvVarSource::ProtonRuntime,
    });

    if let Some(steam_client_path) =
        resolve_steam_client_install_path(request.steam.steam_client_install_path.trim())
    {
        env.push(PreviewEnvVar {
            key: "STEAM_COMPAT_CLIENT_INSTALL_PATH".to_string(),
            value: steam_client_path,
            source: EnvVarSource::ProtonRuntime,
        });
    }

    let proton_verb = if request.launch_trainer_only {
        "runinprefix"
    } else {
        "waitforexitandrun"
    };
    env.push(PreviewEnvVar {
        key: "PROTON_VERB".to_string(),
        value: proton_verb.to_string(),
        source: EnvVarSource::ProtonRuntime,
    });

    let pressure_vessel_paths = collect_pressure_vessel_paths(request).join(":");
    env.push(PreviewEnvVar {
        key: "STEAM_COMPAT_LIBRARY_PATHS".to_string(),
        value: pressure_vessel_paths.clone(),
        source: EnvVarSource::ProtonRuntime,
    });
    env.push(PreviewEnvVar {
        key: "PRESSURE_VESSEL_FILESYSTEMS_RW".to_string(),
        value: pressure_vessel_paths,
        source: EnvVarSource::ProtonRuntime,
    });

    let force_no_umu = force_no_umu_for_launch_request(request);
    let (use_umu, _umu_run_path) = should_use_umu(request, force_no_umu);
    if use_umu {
        let resolved = resolve_launch_proton_path(
            request.runtime.proton_path.trim(),
            request.steam.steam_client_install_path.trim(),
        );
        let dirname = proton_path_dirname(resolved.trim());
        env.push(PreviewEnvVar {
            key: "PROTONPATH".to_string(),
            value: dirname,
            source: EnvVarSource::ProtonRuntime,
        });
    }
}

/// Collects Steam-specific Proton environment variables for `steam_applaunch` launches.
///
/// Uses hardcoded `{compatdata}/pfx` for WINEPREFIX, NOT `resolve_wine_prefix_path()`.
fn collect_steam_proton_environment(request: &LaunchRequest, env: &mut Vec<PreviewEnvVar>) {
    let compatdata = request.steam.compatdata_path.trim();

    env.push(PreviewEnvVar {
        key: "STEAM_COMPAT_DATA_PATH".to_string(),
        value: compatdata.to_string(),
        source: EnvVarSource::SteamProton,
    });

    env.push(PreviewEnvVar {
        key: "STEAM_COMPAT_CLIENT_INSTALL_PATH".to_string(),
        value: request.steam.steam_client_install_path.trim().to_string(),
        source: EnvVarSource::SteamProton,
    });

    env.push(PreviewEnvVar {
        key: "WINEPREFIX".to_string(),
        value: Path::new(compatdata)
            .join("pfx")
            .to_string_lossy()
            .into_owned(),
        source: EnvVarSource::SteamProton,
    });
}

fn merge_optimization_and_custom_preview_env(
    request: &LaunchRequest,
    directives: &LaunchDirectives,
    env: &mut Vec<PreviewEnvVar>,
) {
    for (key, value) in &directives.env {
        upsert_preview_env(env, key, value, EnvVarSource::LaunchOptimization);
    }
    for (key, value) in &request.custom_env_vars {
        upsert_preview_env(env, key, value, EnvVarSource::ProfileCustom);
    }
}

fn merge_custom_preview_env_only(request: &LaunchRequest, env: &mut Vec<PreviewEnvVar>) {
    for (key, value) in &request.custom_env_vars {
        upsert_preview_env(env, key, value, EnvVarSource::ProfileCustom);
    }
}

fn upsert_preview_env(env: &mut Vec<PreviewEnvVar>, key: &str, value: &str, source: EnvVarSource) {
    if let Some(existing) = env.iter_mut().find(|e| e.key == key) {
        existing.value = value.to_string();
        existing.source = source;
    } else {
        env.push(PreviewEnvVar {
            key: key.to_string(),
            value: value.to_string(),
            source,
        });
    }
}

/// Inserts a preview env var only if the key is not already present.
fn insert_preview_env_if_absent(
    env: &mut Vec<PreviewEnvVar>,
    key: &str,
    value: &str,
    source: EnvVarSource,
) {
    if !env.iter().any(|e| e.key == key) {
        env.push(PreviewEnvVar {
            key: key.to_string(),
            value: value.to_string(),
            source,
        });
    }
}

/// Injects `MANGOHUD_CONFIGFILE` (and optionally `MANGOHUD_CONFIG=read_cfg`) into the preview
/// environment vars when the profile has MangoHud config enabled.
///
/// Respects user-supplied `MANGOHUD_CONFIGFILE` in `custom_env_vars` by skipping injection when
/// the key is already present.  The preview path does not check whether the config file exists on
/// disk — it shows what *would* be set.
fn inject_mangohud_config_preview_env(
    env: &mut Vec<PreviewEnvVar>,
    request: &LaunchRequest,
    gamescope_active: bool,
    wrappers_had_mangohud: bool,
) {
    if !request.mangohud.enabled {
        return;
    }

    let user_overrode_configfile = request.custom_env_vars.contains_key("MANGOHUD_CONFIGFILE");

    // Inject MANGOHUD_CONFIGFILE only when the user hasn't explicitly set it.
    if !user_overrode_configfile {
        let profile_name = match request.profile_name.as_deref().filter(|n| !n.is_empty()) {
            Some(n) => n,
            None => {
                // Still fall through to set read_cfg below if gamescope is active.
                if gamescope_active && wrappers_had_mangohud {
                    insert_preview_env_if_absent(
                        env,
                        "MANGOHUD_CONFIG",
                        "read_cfg",
                        EnvVarSource::ProfileCustom,
                    );
                }
                return;
            }
        };

        let base_path = match BaseDirs::new() {
            Some(dirs) => dirs.config_dir().join("crosshook").join("profiles"),
            None => {
                if gamescope_active && wrappers_had_mangohud {
                    insert_preview_env_if_absent(
                        env,
                        "MANGOHUD_CONFIG",
                        "read_cfg",
                        EnvVarSource::ProfileCustom,
                    );
                }
                return;
            }
        };

        let conf_path = crate::profile::mangohud::mangohud_conf_path(&base_path, profile_name);
        let conf_path_str = conf_path.to_string_lossy().into_owned();

        insert_preview_env_if_absent(
            env,
            "MANGOHUD_CONFIGFILE",
            &conf_path_str,
            EnvVarSource::ProfileCustom,
        );
    }

    // Always set read_cfg for gamescope compatibility, regardless of who supplied MANGOHUD_CONFIGFILE.
    if gamescope_active && wrappers_had_mangohud {
        insert_preview_env_if_absent(
            env,
            "MANGOHUD_CONFIG",
            "read_cfg",
            EnvVarSource::ProfileCustom,
        );
    }
}

/// Builds a human-readable command string showing the effective launch command.
fn build_effective_command_string(
    request: &LaunchRequest,
    method: ResolvedLaunchMethod,
    effective_wrappers: &[String],
    gamescope_config: &crate::profile::GamescopeConfig,
    gamescope_active: bool,
) -> Result<String, String> {
    match method {
        ResolvedLaunchMethod::ProtonRun => {
            let mut parts: Vec<String> = Vec::new();
            let force_no_umu = force_no_umu_for_launch_request(request);
            let (use_umu, umu_run_path) = should_use_umu(request, force_no_umu);

            if gamescope_active {
                // Apply MangoHud → mangoapp swap: if wrappers contain "mangohud", remove it and
                // add "--mangoapp" to the gamescope args instead.
                let mut gamescope_args = build_gamescope_args(gamescope_config);
                let wrappers_without_mangohud: Vec<String> = effective_wrappers
                    .iter()
                    .filter(|w| *w != "mangohud")
                    .cloned()
                    .collect();
                let had_mangohud = wrappers_without_mangohud.len() != effective_wrappers.len();
                if had_mangohud {
                    gamescope_args.push("--mangoapp".to_string());
                }

                parts.push("gamescope".to_string());
                parts.extend(gamescope_args);
                parts.push("--".to_string());
                for wrapper in &wrappers_without_mangohud {
                    parts.push(wrapper.clone());
                }
            } else {
                for wrapper in effective_wrappers {
                    parts.push(wrapper.to_string());
                }
            }

            if use_umu {
                parts.push(umu_run_path.unwrap_or_default());
            } else {
                parts.push(request.runtime.proton_path.trim().to_string());
                parts.push("run".to_string());
            }
            if request.launch_trainer_only {
                parts.push(resolve_trainer_launch_path_for_preview(request));
            } else {
                parts.push(request.game_path.trim().to_string());
            }
            Ok(parts.join(" "))
        }
        ResolvedLaunchMethod::SteamApplaunch => {
            let gs = if gamescope_config.enabled {
                Some(gamescope_config)
            } else {
                None
            };
            build_steam_launch_options_command(
                &request.optimizations.enabled_option_ids,
                &request.custom_env_vars,
                gs,
            )
            .map_err(|error| error.to_string())
        }
        ResolvedLaunchMethod::Native => Ok(request.game_path.trim().to_string()),
    }
}

fn resolve_trainer_launch_path_for_preview(request: &LaunchRequest) -> String {
    match request.trainer_loading_mode {
        TrainerLoadingMode::SourceDirectory => request.trainer_host_path.trim().to_string(),
        TrainerLoadingMode::CopyToPrefix => {
            let path = Path::new(request.trainer_host_path.trim());
            let file_stem = path
                .file_stem()
                .map(|segment| segment.to_string_lossy().into_owned())
                .unwrap_or_default();
            let file_name = path
                .file_name()
                .map(|segment| segment.to_string_lossy().into_owned())
                .unwrap_or_default();

            if file_stem.is_empty() || file_name.is_empty() {
                request.trainer_host_path.trim().to_string()
            } else {
                format!("C:\\CrossHook\\StagedTrainers\\{file_stem}\\{file_name}")
            }
        }
    }
}

/// Builds Proton setup details. Returns `None` for native method.
fn build_proton_setup(request: &LaunchRequest, method: &str) -> Option<ProtonSetup> {
    match method {
        METHOD_PROTON_RUN => {
            let resolved_paths =
                resolve_proton_paths(Path::new(request.runtime.prefix_path.trim()));

            let steam_client =
                resolve_steam_client_install_path(request.steam.steam_client_install_path.trim())
                    .unwrap_or_default();

            let force_no_umu = force_no_umu_for_launch_request(request);
            let (use_umu, umu_run_path) = should_use_umu(request, force_no_umu);
            let umu_run_path_field = if use_umu { umu_run_path } else { None };

            Some(ProtonSetup {
                wine_prefix_path: resolved_paths
                    .wine_prefix_path
                    .to_string_lossy()
                    .into_owned(),
                compat_data_path: resolved_paths
                    .compat_data_path
                    .to_string_lossy()
                    .into_owned(),
                steam_client_install_path: steam_client,
                proton_executable: request.runtime.proton_path.trim().to_string(),
                umu_run_path: umu_run_path_field,
            })
        }
        METHOD_STEAM_APPLAUNCH => {
            // Steam opt-out: never reflect umu in the preview setup.
            let compatdata = request.steam.compatdata_path.trim();

            Some(ProtonSetup {
                wine_prefix_path: Path::new(compatdata)
                    .join("pfx")
                    .to_string_lossy()
                    .into_owned(),
                compat_data_path: compatdata.to_string(),
                steam_client_install_path: request
                    .steam
                    .steam_client_install_path
                    .trim()
                    .to_string(),
                proton_executable: request.steam.proton_path.trim().to_string(),
                umu_run_path: None,
            })
        }
        _ => None,
    }
}

/// Builds trainer info. Returns `None` if trainer path is empty or native method.
///
/// For `copy_to_prefix` mode, computes the staged path via string manipulation
/// without calling `stage_trainer_into_prefix()` (which has side effects).
fn build_trainer_info(request: &LaunchRequest, method: &str) -> Option<PreviewTrainerInfo> {
    if method == METHOD_NATIVE {
        return None;
    }

    let trainer_path = request.trainer_path.trim();
    if trainer_path.is_empty() {
        return None;
    }

    let host_path = request.trainer_host_path.trim().to_string();
    let loading_mode = request.trainer_loading_mode;

    let staged_path = if request.trainer_loading_mode == TrainerLoadingMode::CopyToPrefix {
        let path = Path::new(request.trainer_host_path.trim());
        let file_stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let file_name = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !file_stem.is_empty() && !file_name.is_empty() {
            Some(format!(
                "C:\\CrossHook\\StagedTrainers\\{file_stem}\\{file_name}"
            ))
        } else {
            None
        }
    } else {
        None
    };

    Some(PreviewTrainerInfo {
        path: trainer_path.to_string(),
        host_path,
        loading_mode,
        staged_path,
    })
}

/// Resolves the working directory shown in preview output.
///
/// `steam_applaunch` intentionally reports no working directory because
/// the launch path does not apply one at runtime.
fn resolve_working_directory(request: &LaunchRequest, method: &str) -> String {
    if method == METHOD_STEAM_APPLAUNCH {
        return String::new();
    }

    let configured = request.runtime.working_directory.trim();
    if !configured.is_empty() {
        return configured.to_string();
    }

    let game_path = Path::new(request.game_path.trim());
    game_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn append_preview_error(target: &mut Option<String>, message: String) {
    match target {
        Some(existing) => {
            if existing != &message {
                existing.push('\n');
                existing.push_str(&message);
            }
        }
        None => *target = Some(message),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::*;
    use crate::launch::request::{
        LaunchOptimizationsRequest, RuntimeLaunchConfig, SteamLaunchConfig, METHOD_NATIVE,
        METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH,
    };
    use crate::profile::TrainerLoadingMode;
    use serde_json::json;

    // -- Fixture helpers (mirrors request.rs test factories) --

    struct RequestFixture {
        _temp_dir: tempfile::TempDir,
        game_path: String,
        trainer_path: String,
        compatdata_path: String,
        proton_path: String,
        steam_client_install_path: String,
    }

    fn write_executable_file(path: &Path) {
        fs::write(path, b"test").expect("write file");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(path).expect("metadata").permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).expect("chmod");
        }
    }

    fn fixture() -> RequestFixture {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let compatdata = temp_dir.path().join("compat");
        let proton = temp_dir.path().join("proton");
        let game = temp_dir.path().join("game.sh");
        let trainer = temp_dir.path().join("trainer.exe");
        let steam_client = temp_dir.path().join("steam");

        fs::create_dir_all(&compatdata).expect("compatdata dir");
        fs::create_dir_all(&steam_client).expect("steam client dir");
        write_executable_file(&proton);
        write_executable_file(&game);
        fs::write(&trainer, b"trainer").expect("trainer file");

        RequestFixture {
            _temp_dir: temp_dir,
            game_path: game.to_string_lossy().into_owned(),
            trainer_path: trainer.to_string_lossy().into_owned(),
            compatdata_path: compatdata.to_string_lossy().into_owned(),
            proton_path: proton.to_string_lossy().into_owned(),
            steam_client_install_path: steam_client.to_string_lossy().into_owned(),
        }
    }

    fn steam_request() -> (tempfile::TempDir, LaunchRequest) {
        let RequestFixture {
            _temp_dir,
            game_path,
            trainer_path,
            compatdata_path,
            proton_path,
            steam_client_install_path,
        } = fixture();
        (
            _temp_dir,
            LaunchRequest {
                method: METHOD_STEAM_APPLAUNCH.to_string(),
                game_path,
                trainer_path: trainer_path.clone(),
                trainer_host_path: trainer_path,
                trainer_loading_mode: TrainerLoadingMode::SourceDirectory,
                steam: SteamLaunchConfig {
                    app_id: "12345".to_string(),
                    compatdata_path,
                    proton_path,
                    steam_client_install_path,
                },
                runtime: RuntimeLaunchConfig::default(),
                optimizations: LaunchOptimizationsRequest::default(),
                launch_trainer_only: false,
                launch_game_only: false,
                profile_name: None,
                ..Default::default()
            },
        )
    }

    fn proton_request() -> (tempfile::TempDir, LaunchRequest) {
        let (temp_dir, mut request) = steam_request();
        request.method = METHOD_PROTON_RUN.to_string();
        request.game_path = request.game_path.replace("game.sh", "game.exe");
        fs::write(&request.game_path, b"game").expect("game exe");
        request.runtime = RuntimeLaunchConfig {
            prefix_path: request.steam.compatdata_path.clone(),
            proton_path: request.steam.proton_path.clone(),
            working_directory: String::new(),
            steam_app_id: String::new(),
            umu_game_id: String::new(),
        };
        request.steam = SteamLaunchConfig::default();
        (temp_dir, request)
    }

    fn native_request() -> (tempfile::TempDir, LaunchRequest) {
        let (temp_dir, mut request) = steam_request();
        request.method = METHOD_NATIVE.to_string();
        request.trainer_path.clear();
        request.trainer_host_path.clear();
        request.steam = SteamLaunchConfig::default();
        (temp_dir, request)
    }

    // -- Tests --

    #[test]
    fn preview_shows_resolved_method_for_steam_applaunch() {
        let (_td, request) = steam_request();
        let preview = build_launch_preview(&request).expect("preview");
        assert_eq!(
            preview.resolved_method,
            ResolvedLaunchMethod::SteamApplaunch
        );
    }

    #[test]
    fn preview_shows_resolved_method_for_proton_run() {
        let (_td, request) = proton_request();
        let preview = build_launch_preview(&request).expect("preview");
        assert_eq!(preview.resolved_method, ResolvedLaunchMethod::ProtonRun);
    }

    #[test]
    fn preview_shows_resolved_method_for_native() {
        let (_td, request) = native_request();
        let preview = build_launch_preview(&request).expect("preview");
        assert_eq!(preview.resolved_method, ResolvedLaunchMethod::Native);
    }

    #[test]
    fn preview_validation_passes_for_valid_request() {
        let (_td, request) = steam_request();
        let preview = build_launch_preview(&request).expect("preview");
        assert!(
            preview.validation.issues.is_empty(),
            "expected validation to pass, issues: {:?}",
            preview.validation.issues
        );
        assert!(
            preview.validation.issues.is_empty(),
            "expected no issues, got: {:?}",
            preview.validation.issues
        );
    }

    #[test]
    fn preview_validation_collects_multiple_issues() {
        let (_td, mut request) = steam_request();
        request.game_path.clear();
        request.steam.app_id.clear();
        request.steam.compatdata_path.clear();
        request.steam.proton_path.clear();

        let preview = build_launch_preview(&request).expect("preview");
        assert!(
            !preview.validation.issues.is_empty(),
            "expected validation to fail"
        );
        assert!(
            preview.validation.issues.len() >= 4,
            "expected at least 4 issues, got {}: {:?}",
            preview.validation.issues.len(),
            preview.validation.issues
        );
    }

    #[test]
    fn preview_returns_partial_results_on_directive_failure() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let _scoped_path =
            crate::launch::test_support::ScopedCommandSearchPath::new(temp_dir.path());

        let (_td, mut request) = proton_request();
        // Enable an optimization that requires a missing wrapper binary.
        request.optimizations.enabled_option_ids = vec!["show_mangohud_overlay".to_string()];

        let preview = build_launch_preview(&request).expect("preview");

        // Directives failed — error is captured, env/command are None.
        assert!(
            preview.directives_error.is_some(),
            "expected directives_error to be Some"
        );
        assert!(
            preview.environment.is_none(),
            "expected environment to be None when directives fail"
        );
        assert!(
            preview.effective_command.is_none(),
            "expected effective_command to be None when directives fail"
        );

        // Validation and game info should still be populated.
        assert!(!preview.game_executable.is_empty());
        // The validation should have collected the directive error as an issue too.
        assert!(
            !preview.validation.issues.is_empty(),
            "expected validation issues for missing wrapper"
        );
    }

    #[test]
    fn preview_trainer_info_with_copy_to_prefix() {
        let (_td, mut request) = proton_request();
        request.trainer_loading_mode = TrainerLoadingMode::CopyToPrefix;
        request.trainer_host_path = "/home/user/trainers/MyTrainer.exe".to_string();

        let preview = build_launch_preview(&request).expect("preview");
        let trainer = preview.trainer.expect("trainer info should be present");

        assert_eq!(trainer.loading_mode, TrainerLoadingMode::CopyToPrefix);
        assert_eq!(
            trainer.staged_path,
            Some("C:\\CrossHook\\StagedTrainers\\MyTrainer\\MyTrainer.exe".to_string())
        );
    }

    #[test]
    fn preview_hides_proton_for_native() {
        let (_td, request) = native_request();
        let preview = build_launch_preview(&request).expect("preview");
        assert!(
            preview.proton_setup.is_none(),
            "expected proton_setup to be None for native, got: {:?}",
            preview.proton_setup
        );
    }

    #[test]
    fn preview_includes_steam_launch_options() {
        let (_td, request) = steam_request();
        let preview = build_launch_preview(&request).expect("preview");

        // With no optimizations enabled, steam launch options should still be
        // populated (the bare "%command%" string).
        assert!(
            preview.steam_launch_options.is_some(),
            "expected steam_launch_options for steam_applaunch"
        );
        assert_eq!(preview.steam_launch_options.as_deref(), Some("%command%"));
    }

    #[test]
    fn preview_hides_working_directory_for_steam_applaunch() {
        let (_td, request) = steam_request();
        let preview = build_launch_preview(&request).expect("preview");

        assert!(
            preview.working_directory.is_empty(),
            "expected no working directory for steam_applaunch preview, got: {:?}",
            preview.working_directory
        );
    }

    #[test]
    fn preview_generated_at_is_recent() {
        let (_td, request) = steam_request();
        let preview = build_launch_preview(&request).expect("preview");

        let parsed = chrono::DateTime::parse_from_rfc3339(&preview.generated_at);
        assert!(
            parsed.is_ok(),
            "generated_at '{}' should parse as ISO 8601 / RFC 3339",
            preview.generated_at
        );

        let generated = parsed.unwrap();
        let now = chrono::Utc::now();
        let age = now.signed_duration_since(generated);
        assert!(
            age.num_seconds() < 10,
            "generated_at should be within 10 seconds of now, got age: {}s",
            age.num_seconds()
        );
    }

    #[test]
    fn preview_runtime_environment_matches_proton_setup_for_compat_root() {
        let (_td, request) = proton_request();
        let preview = build_launch_preview(&request).expect("preview");
        let environment = preview.environment.expect("environment");
        let proton_setup = preview.proton_setup.expect("proton setup");

        let wine_prefix = environment
            .iter()
            .find(|variable| variable.key == "WINEPREFIX")
            .expect("WINEPREFIX");
        let compat_path = environment
            .iter()
            .find(|variable| variable.key == "STEAM_COMPAT_DATA_PATH")
            .expect("STEAM_COMPAT_DATA_PATH");

        assert_eq!(wine_prefix.value, proton_setup.wine_prefix_path);
        assert_eq!(compat_path.value, proton_setup.compat_data_path);
    }

    #[test]
    fn preview_runtime_environment_matches_proton_setup_for_pfx_root() {
        let (_td, mut request) = proton_request();
        let prefix_path = Path::new(&request.runtime.prefix_path).join("pfx");
        fs::create_dir_all(&prefix_path).expect("create pfx dir");
        request.runtime.prefix_path = prefix_path.to_string_lossy().into_owned();

        let preview = build_launch_preview(&request).expect("preview");
        let environment = preview.environment.expect("environment");
        let proton_setup = preview.proton_setup.expect("proton setup");

        let wine_prefix = environment
            .iter()
            .find(|variable| variable.key == "WINEPREFIX")
            .expect("WINEPREFIX");
        let compat_path = environment
            .iter()
            .find(|variable| variable.key == "STEAM_COMPAT_DATA_PATH")
            .expect("STEAM_COMPAT_DATA_PATH");

        assert_eq!(wine_prefix.value, proton_setup.wine_prefix_path);
        assert_eq!(compat_path.value, proton_setup.compat_data_path);
        assert_eq!(
            compat_path.value,
            prefix_path
                .parent()
                .expect("compatdata parent")
                .to_string_lossy()
                .into_owned()
        );
    }

    #[test]
    fn preview_serializes_typed_fields_as_snake_case_strings() {
        let (_td, mut request) = proton_request();
        request.trainer_loading_mode = TrainerLoadingMode::CopyToPrefix;

        let preview = build_launch_preview(&request).expect("preview");
        let value = serde_json::to_value(&preview).expect("serialize preview");

        assert_eq!(value["resolved_method"], json!("proton_run"));
        assert_eq!(value["trainer"]["loading_mode"], json!("copy_to_prefix"));
    }

    #[test]
    fn preview_surfaces_steam_launch_option_failures_without_fake_command() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let _scoped_path =
            crate::launch::test_support::ScopedCommandSearchPath::new(temp_dir.path());

        let (_td, mut request) = steam_request();
        request.optimizations.enabled_option_ids = vec!["show_mangohud_overlay".to_string()];

        let preview = build_launch_preview(&request).expect("preview");

        assert!(preview.effective_command.is_none());
        assert!(preview.steam_launch_options.is_none());
        assert!(
            preview
                .directives_error
                .as_deref()
                .is_some_and(|error| error.contains("mangohud")),
            "expected directives_error to mention the missing wrapper, got {:?}",
            preview.directives_error
        );
    }

    #[test]
    fn preview_proton_dxvk_custom_matches_runtime_command_env() {
        use std::collections::BTreeMap;

        let (_td, mut request) = proton_request();
        request.optimizations.enabled_option_ids = vec!["enable_dxvk_async".to_string()];
        request.custom_env_vars = BTreeMap::from([("DXVK_ASYNC".to_string(), "0".to_string())]);

        let preview = build_launch_preview(&request).expect("preview");
        let log_path = _td.path().join("parity.log");
        let command = crate::launch::script_runner::build_proton_game_command(&request, &log_path)
            .expect("command");

        let dxvk = preview
            .environment
            .as_ref()
            .expect("environment")
            .iter()
            .find(|v| v.key == "DXVK_ASYNC")
            .expect("DXVK_ASYNC in preview");
        assert_eq!(dxvk.value, "0");
        assert_eq!(dxvk.source, EnvVarSource::ProfileCustom);

        let cmd_val = command
            .as_std()
            .get_envs()
            .find_map(|(k, v)| {
                (k == std::ffi::OsStr::new("DXVK_ASYNC"))
                    .then(|| v.map(|x| x.to_string_lossy().into_owned()))
            })
            .flatten();
        assert_eq!(cmd_val.as_deref(), Some("0"));
    }

    #[test]
    fn preview_steam_launch_options_string_matches_core_builder_with_custom_merge() {
        use std::collections::BTreeMap;

        let (_td, mut request) = steam_request();
        request.optimizations.enabled_option_ids = vec!["enable_dxvk_async".to_string()];
        request.custom_env_vars = BTreeMap::from([("DXVK_ASYNC".to_string(), "0".to_string())]);

        let preview = build_launch_preview(&request).expect("preview");
        let expected = build_steam_launch_options_command(
            &request.optimizations.enabled_option_ids,
            &request.custom_env_vars,
            None,
        )
        .expect("steam line");

        assert_eq!(
            preview.steam_launch_options.as_deref(),
            Some(expected.as_str())
        );

        let dxvk = preview
            .environment
            .as_ref()
            .expect("environment")
            .iter()
            .find(|v| v.key == "DXVK_ASYNC")
            .expect("DXVK_ASYNC");
        assert_eq!(dxvk.value, "0");
        assert_eq!(dxvk.source, EnvVarSource::ProfileCustom);
    }

    #[test]
    fn preview_steam_gamescope_active_includes_gamescope_in_command() {
        let (_td, mut request) = steam_request();
        request.gamescope = crate::profile::GamescopeConfig {
            enabled: true,
            internal_width: Some(2560),
            internal_height: Some(1440),
            fullscreen: true,
            ..Default::default()
        };

        let preview = build_launch_preview(&request).expect("preview");
        let steam_opts = preview
            .steam_launch_options
            .as_deref()
            .expect("steam_launch_options");
        assert!(
            steam_opts.starts_with("gamescope"),
            "steam launch options should start with gamescope: {steam_opts}"
        );
        assert!(
            steam_opts.contains("-w 2560 -h 1440 -f"),
            "should contain gamescope args: {steam_opts}"
        );
        assert!(
            steam_opts.contains("-- %command%"),
            "should contain separator before %%command%%: {steam_opts}"
        );

        let effective = preview
            .effective_command
            .as_deref()
            .expect("effective_command");
        assert!(
            effective.starts_with("gamescope"),
            "effective command should also contain gamescope: {effective}"
        );
    }

    #[test]
    fn preview_steam_gamescope_mangohud_swap() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let mangohud_path = temp_dir.path().join("mangohud");
        write_executable_file(&mangohud_path);
        let _command_search_path =
            crate::launch::test_support::ScopedCommandSearchPath::new(temp_dir.path());

        let (_td, mut request) = steam_request();
        request.gamescope = crate::profile::GamescopeConfig {
            enabled: true,
            fullscreen: true,
            ..Default::default()
        };
        request.optimizations.enabled_option_ids = vec!["show_mangohud_overlay".to_string()];

        let preview = build_launch_preview(&request).expect("preview");
        let steam_opts = preview
            .steam_launch_options
            .as_deref()
            .expect("steam_launch_options");
        assert!(
            steam_opts.contains("--mangoapp"),
            "should contain --mangoapp: {steam_opts}"
        );
        // mangohud should not appear as a separate wrapper token between -- and %command%
        let after_separator = steam_opts.split("-- ").last().unwrap_or("");
        assert!(
            !after_separator.contains("mangohud"),
            "mangohud should not appear as wrapper after --: {steam_opts}"
        );
    }

    #[test]
    fn preview_trainer_only_uses_trainer_gamescope_and_trainer_path() {
        let (_td, mut request) = proton_request();
        request.launch_trainer_only = true;
        request.launch_game_only = false;
        request.gamescope = crate::profile::GamescopeConfig::default();
        request.trainer_gamescope = Some(crate::profile::GamescopeConfig {
            enabled: true,
            internal_width: Some(1024),
            internal_height: Some(576),
            ..Default::default()
        });

        let preview = build_launch_preview(&request).expect("preview");
        assert!(preview.gamescope_active);
        let command = preview
            .effective_command
            .as_deref()
            .expect("effective command");
        assert!(
            command.starts_with("gamescope"),
            "expected gamescope in: {command}"
        );
        assert!(
            command.contains(request.trainer_host_path.as_str()),
            "expected trainer host path in: {command}"
        );
        assert!(
            !command.contains(request.game_path.as_str()),
            "trainer-only command should not contain game path: {command}"
        );
    }

    #[test]
    fn preview_trainer_only_falls_back_to_main_gamescope_when_trainer_disabled() {
        let (_td, mut request) = proton_request();
        request.launch_trainer_only = true;
        request.launch_game_only = false;
        request.gamescope = crate::profile::GamescopeConfig {
            enabled: true,
            fullscreen: true,
            internal_width: Some(1920),
            internal_height: Some(1080),
            ..Default::default()
        };
        request.trainer_gamescope = Some(crate::profile::GamescopeConfig::default());

        let preview = build_launch_preview(&request).expect("preview");
        assert!(
            preview.gamescope_active,
            "expected fallback gamescope to be active"
        );
        let command = preview
            .effective_command
            .as_deref()
            .expect("effective command");
        assert!(
            command.contains("-w 1920 -h 1080"),
            "expected auto-generated trainer gamescope resolution in: {command}"
        );
        assert!(
            !command.split_whitespace().any(|token| token == "-f"),
            "auto-generated trainer gamescope should not force fullscreen: {command}"
        );
    }

    #[test]
    fn preview_trainer_only_auto_derives_windowed_gamescope_when_trainer_gamescope_is_none() {
        let (_td, mut request) = proton_request();
        request.launch_trainer_only = true;
        request.launch_game_only = false;
        request.gamescope = crate::profile::GamescopeConfig {
            enabled: true,
            fullscreen: true,
            output_width: Some(1920),
            output_height: Some(1080),
            ..Default::default()
        };
        request.trainer_gamescope = None;

        let preview = build_launch_preview(&request).expect("preview");
        assert!(
            preview.gamescope_active,
            "expected auto-derived gamescope to be active"
        );
        let command = preview
            .effective_command
            .as_deref()
            .expect("effective command");
        assert!(
            command.contains("-W 1920 -H 1080"),
            "expected auto-derived trainer gamescope output resolution in: {command}"
        );
        assert!(
            !command.split_whitespace().any(|token| token == "-f"),
            "auto-derived trainer gamescope should not force fullscreen: {command}"
        );
    }

    #[test]
    fn preview_proton_verb_is_waitforexitandrun_for_game_and_runinprefix_for_trainer() {
        // Game launch: PROTON_VERB should be "waitforexitandrun"
        let (_td, request) = proton_request();
        let preview = build_launch_preview(&request).expect("preview");
        let env = preview.environment.expect("environment");
        let verb = env
            .iter()
            .find(|v| v.key == "PROTON_VERB")
            .expect("PROTON_VERB in game preview env");
        assert_eq!(verb.value, "waitforexitandrun");
        assert_eq!(verb.source, EnvVarSource::ProtonRuntime);

        // Trainer-only launch: PROTON_VERB should be "runinprefix"
        let (_td2, mut trainer_request) = proton_request();
        trainer_request.launch_trainer_only = true;
        trainer_request.launch_game_only = false;
        let trainer_preview = build_launch_preview(&trainer_request).expect("trainer preview");
        let trainer_env = trainer_preview.environment.expect("trainer environment");
        let trainer_verb = trainer_env
            .iter()
            .find(|v| v.key == "PROTON_VERB")
            .expect("PROTON_VERB in trainer preview env");
        assert_eq!(trainer_verb.value, "runinprefix");
        assert_eq!(trainer_verb.source, EnvVarSource::ProtonRuntime);
    }

    #[test]
    fn preview_runtime_proton_env_includes_pressure_vessel_paths() {
        let (_td, mut request) = proton_request();
        let shared_root = Path::new(&request.game_path)
            .parent()
            .expect("game parent")
            .join("pressure-vessel");
        let game_dir = shared_root.join("game");
        let trainer_dir = shared_root.join("trainer");
        let working_dir = shared_root.join("working");

        fs::create_dir_all(&game_dir).expect("game dir");
        fs::create_dir_all(&trainer_dir).expect("trainer dir");
        fs::create_dir_all(&working_dir).expect("working dir");

        request.game_path = game_dir.join("game.exe").to_string_lossy().into_owned();
        request.trainer_host_path = trainer_dir
            .join("trainer.exe")
            .to_string_lossy()
            .into_owned();
        request.runtime.working_directory = working_dir.to_string_lossy().into_owned();
        fs::write(&request.game_path, b"game").expect("game exe");
        fs::write(&request.trainer_host_path, b"trainer").expect("trainer exe");

        let preview = build_launch_preview(&request).expect("preview");
        let env = preview.environment.expect("environment");
        let expected_paths = format!(
            "{}:{}:{}",
            game_dir.to_string_lossy(),
            trainer_dir.to_string_lossy(),
            working_dir.to_string_lossy()
        );

        let steam_compat_library_paths = env
            .iter()
            .find(|var| var.key == "STEAM_COMPAT_LIBRARY_PATHS")
            .expect("STEAM_COMPAT_LIBRARY_PATHS in preview env");
        assert_eq!(steam_compat_library_paths.value, expected_paths);
        assert_eq!(
            steam_compat_library_paths.source,
            EnvVarSource::ProtonRuntime
        );

        let pressure_vessel_filesystems_rw = env
            .iter()
            .find(|var| var.key == "PRESSURE_VESSEL_FILESYSTEMS_RW")
            .expect("PRESSURE_VESSEL_FILESYSTEMS_RW in preview env");
        assert_eq!(pressure_vessel_filesystems_rw.value, expected_paths);
        assert_eq!(
            pressure_vessel_filesystems_rw.source,
            EnvVarSource::ProtonRuntime
        );
    }

    #[test]
    fn preview_runtime_proton_env_pressure_vessel_omits_trainer_under_copy_to_prefix() {
        let (_td, mut request) = proton_request();
        let shared_root = Path::new(&request.game_path)
            .parent()
            .expect("game parent")
            .join("pressure-vessel-copy");
        let game_dir = shared_root.join("game");
        let trainer_dir = shared_root.join("trainer");
        let working_dir = shared_root.join("working");

        fs::create_dir_all(&game_dir).expect("game dir");
        fs::create_dir_all(&trainer_dir).expect("trainer dir");
        fs::create_dir_all(&working_dir).expect("working dir");

        request.trainer_loading_mode = TrainerLoadingMode::CopyToPrefix;
        request.game_path = game_dir.join("game.exe").to_string_lossy().into_owned();
        request.trainer_host_path = trainer_dir
            .join("trainer.exe")
            .to_string_lossy()
            .into_owned();
        request.runtime.working_directory = working_dir.to_string_lossy().into_owned();
        fs::write(&request.game_path, b"game").expect("game exe");
        fs::write(&request.trainer_host_path, b"trainer").expect("trainer exe");

        let preview = build_launch_preview(&request).expect("preview");
        let env = preview.environment.expect("environment");
        let expected_paths = format!(
            "{}:{}",
            game_dir.to_string_lossy(),
            working_dir.to_string_lossy()
        );

        let steam_compat_library_paths = env
            .iter()
            .find(|var| var.key == "STEAM_COMPAT_LIBRARY_PATHS")
            .expect("STEAM_COMPAT_LIBRARY_PATHS in preview env");
        assert_eq!(steam_compat_library_paths.value, expected_paths);
        assert!(
            !steam_compat_library_paths
                .value
                .contains(trainer_dir.to_string_lossy().as_ref()),
            "copy_to_prefix should omit trainer dir: {}",
            steam_compat_library_paths.value
        );

        let pressure_vessel_filesystems_rw = env
            .iter()
            .find(|var| var.key == "PRESSURE_VESSEL_FILESYSTEMS_RW")
            .expect("PRESSURE_VESSEL_FILESYSTEMS_RW in preview env");
        assert_eq!(pressure_vessel_filesystems_rw.value, expected_paths);
        assert!(
            !pressure_vessel_filesystems_rw
                .value
                .contains(trainer_dir.to_string_lossy().as_ref()),
            "copy_to_prefix should omit trainer dir: {}",
            pressure_vessel_filesystems_rw.value
        );
    }

    #[test]
    fn preview_command_string_uses_umu_run_when_use_umu() {
        let dir = tempfile::tempdir().unwrap();
        let umu_stub = dir.path().join("umu-run");
        std::fs::write(&umu_stub, "#!/bin/sh\nexit 0\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&umu_stub, std::fs::Permissions::from_mode(0o755)).unwrap();
        let _guard = crate::launch::test_support::ScopedCommandSearchPath::new(dir.path());

        let mut request = LaunchRequest {
            method: METHOD_PROTON_RUN.to_string(),
            game_path: "/tmp/game.exe".to_string(),
            umu_preference: crate::settings::UmuPreference::Umu,
            ..Default::default()
        };
        request.runtime.proton_path = "/opt/proton/GE-Proton9-20/proton".to_string();

        let preview = build_launch_preview(&request).unwrap();
        let command = preview.effective_command.unwrap();
        assert!(
            command.contains("umu-run"),
            "expected 'umu-run' in command, got: {command}"
        );
        assert!(
            !command.contains(" run /tmp/game.exe"),
            "no 'run' subcommand expected: {command}"
        );
    }

    #[test]
    fn preview_pushes_protonpath_env_when_use_umu() {
        let dir = tempfile::tempdir().unwrap();
        let umu_stub = dir.path().join("umu-run");
        std::fs::write(&umu_stub, "#!/bin/sh\nexit 0\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&umu_stub, std::fs::Permissions::from_mode(0o755)).unwrap();
        let _guard = crate::launch::test_support::ScopedCommandSearchPath::new(dir.path());

        let mut request = LaunchRequest {
            method: METHOD_PROTON_RUN.to_string(),
            game_path: "/tmp/game.exe".to_string(),
            umu_preference: crate::settings::UmuPreference::Umu,
            ..Default::default()
        };
        request.runtime.proton_path = "/opt/proton/GE-Proton9-20/proton".to_string();

        let preview = build_launch_preview(&request).unwrap();
        let env = preview.environment.unwrap();
        let protonpath = env
            .iter()
            .find(|e| e.key == "PROTONPATH")
            .expect("expected PROTONPATH env entry");
        assert_eq!(protonpath.value, "/opt/proton/GE-Proton9-20");
        assert!(matches!(protonpath.source, EnvVarSource::ProtonRuntime));
    }

    #[test]
    fn preview_steam_branch_does_not_push_protonpath() {
        let dir = tempfile::tempdir().unwrap();
        let umu_stub = dir.path().join("umu-run");
        std::fs::write(&umu_stub, "#!/bin/sh\nexit 0\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&umu_stub, std::fs::Permissions::from_mode(0o755)).unwrap();
        let _guard = crate::launch::test_support::ScopedCommandSearchPath::new(dir.path());

        let mut request = LaunchRequest {
            method: METHOD_STEAM_APPLAUNCH.to_string(),
            game_path: "/tmp/game.exe".to_string(),
            umu_preference: crate::settings::UmuPreference::Umu,
            ..Default::default()
        };
        request.steam.app_id = "70".to_string();
        request.steam.compatdata_path = "/tmp/compat".to_string();
        request.steam.proton_path = "/opt/steam/proton/proton".to_string();

        let preview = build_launch_preview(&request).unwrap();
        let env = preview.environment.unwrap();
        assert!(
            env.iter().find(|e| e.key == "PROTONPATH").is_none(),
            "Steam branch must not push PROTONPATH"
        );
        assert!(
            preview.proton_setup.unwrap().umu_run_path.is_none(),
            "Steam ProtonSetup.umu_run_path must be None"
        );
    }

    #[test]
    fn preview_proton_setup_umu_run_path_none_when_preference_is_proton() {
        let dir = tempfile::tempdir().unwrap();
        let umu_stub = dir.path().join("umu-run");
        std::fs::write(&umu_stub, "#!/bin/sh\nexit 0\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&umu_stub, std::fs::Permissions::from_mode(0o755)).unwrap();
        let _guard = crate::launch::test_support::ScopedCommandSearchPath::new(dir.path());

        let mut request = LaunchRequest {
            method: METHOD_PROTON_RUN.to_string(),
            game_path: "/tmp/game.exe".to_string(),
            umu_preference: crate::settings::UmuPreference::Proton,
            ..Default::default()
        };
        request.runtime.proton_path = "/opt/proton/GE-Proton9-20/proton".to_string();

        let preview = build_launch_preview(&request).unwrap();
        assert!(preview.proton_setup.unwrap().umu_run_path.is_none());
    }

    // -- CSV coverage tests (C6.3) --

    // Serialize all tests that mutate process-global env vars (HOME, XDG_DATA_HOME, XDG_DATA_DIRS).
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    // Minimal fixture CSV — Ghost of Tsushima (546590) present; Witcher 3 (292030) absent.
    const FIXTURE_CSV: &str = "\
TITLE,STORE,CODENAME,UMU_ID,COMMON ACRONYM (Optional),NOTE (Optional),EXE_STRINGS (Optional)
Ghost of Tsushima,steam,546590,umu-546590,GoT,,ghostoftsushima.exe
";

    #[test]
    fn preview_reports_csv_coverage_found_when_app_id_matches() {
        let _env = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        // Place the fixture at data_local_dir()/crosshook/umu-database.csv — priority 1 in
        // resolve_umu_database_path, found before any system /usr/share/... paths.
        let xdg_data_home = tmp.path().join("local_share");
        let csv_dir = xdg_data_home.join("crosshook");
        std::fs::create_dir_all(&csv_dir).unwrap();
        std::fs::write(csv_dir.join("umu-database.csv"), FIXTURE_CSV).unwrap();
        std::env::set_var("HOME", tmp.path());
        std::env::set_var("XDG_DATA_HOME", &xdg_data_home);
        std::env::set_var("XDG_DATA_DIRS", "");
        crate::umu_database::coverage::clear_cache_for_test();

        let (_td, mut request) = proton_request();
        request.steam.app_id = "546590".to_string();
        let preview = build_launch_preview(&request).unwrap();
        let umu = preview
            .umu_decision
            .as_ref()
            .expect("umu_decision populated");
        assert_eq!(umu.csv_coverage, crate::umu_database::CsvCoverage::Found);
    }

    #[test]
    fn preview_reports_csv_coverage_missing_when_app_id_absent() {
        let _env = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        // Same fixture CSV as the Found test — Witcher 3 (292030) is the motivating missing case
        // from issue #262 (proton-cachyos STEAM_COMPAT_APP_ID override side-effect).
        let xdg_data_home = tmp.path().join("local_share");
        let csv_dir = xdg_data_home.join("crosshook");
        std::fs::create_dir_all(&csv_dir).unwrap();
        std::fs::write(csv_dir.join("umu-database.csv"), FIXTURE_CSV).unwrap();
        std::env::set_var("HOME", tmp.path());
        std::env::set_var("XDG_DATA_HOME", &xdg_data_home);
        std::env::set_var("XDG_DATA_DIRS", "");
        crate::umu_database::coverage::clear_cache_for_test();

        let (_td, mut request) = proton_request();
        request.steam.app_id = "292030".to_string(); // Witcher 3 — absent from fixture
        let preview = build_launch_preview(&request).unwrap();
        let umu = preview
            .umu_decision
            .as_ref()
            .expect("umu_decision populated");
        assert_eq!(umu.csv_coverage, crate::umu_database::CsvCoverage::Missing);
    }

    #[test]
    fn preview_reports_csv_coverage_unknown_when_no_csv_source() {
        // Skip on hosts where a system-level CSV exists and cannot be overridden —
        // resolve_umu_database_path checks hardcoded /usr/share/... paths before XDG_DATA_DIRS,
        // and we cannot redirect those.
        let system_csvs = [
            "/usr/share/umu-protonfixes/umu-database.csv",
            "/usr/share/umu/umu-database.csv",
            "/opt/umu-launcher/umu-protonfixes/umu-database.csv",
        ];
        if system_csvs
            .iter()
            .any(|p| std::fs::metadata(p).map(|m| m.is_file()).unwrap_or(false))
        {
            eprintln!("skip: host has a system umu-database CSV — cannot isolate Unknown case");
            return;
        }

        let _env = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        // Point HOME and XDG_DATA_HOME at an empty tempdir — no CSV anywhere reachable.
        std::env::set_var("HOME", tmp.path());
        std::env::set_var("XDG_DATA_HOME", tmp.path().join("local_share"));
        std::env::set_var("XDG_DATA_DIRS", "");
        crate::umu_database::coverage::clear_cache_for_test();

        let (_td, mut request) = proton_request();
        request.steam.app_id = "546590".to_string();
        let preview = build_launch_preview(&request).unwrap();
        let umu = preview
            .umu_decision
            .as_ref()
            .expect("umu_decision populated");
        assert_eq!(umu.csv_coverage, crate::umu_database::CsvCoverage::Unknown);
    }

    #[test]
    fn auto_preference_preview_reports_using_umu_when_umu_run_present() {
        let dir = tempfile::tempdir().unwrap();
        let umu_stub = dir.path().join("umu-run");
        std::fs::write(&umu_stub, "#!/bin/sh\nexit 0\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&umu_stub, std::fs::Permissions::from_mode(0o755)).unwrap();
        let _guard = crate::launch::test_support::ScopedCommandSearchPath::new(dir.path());

        let mut request = LaunchRequest {
            method: METHOD_PROTON_RUN.to_string(),
            game_path: "/tmp/game.exe".to_string(),
            umu_preference: crate::settings::UmuPreference::Auto,
            ..Default::default()
        };
        request.runtime.proton_path = "/opt/proton/GE-Proton9-20/proton".to_string();

        let preview = build_launch_preview(&request).unwrap();
        let decision = preview.umu_decision.as_ref().unwrap();
        assert!(decision.will_use_umu, "Auto + umu-run present must use umu");
        assert!(
            decision.reason.starts_with("using umu-run at "),
            "expected reason starting with 'using umu-run at ', got: {}",
            decision.reason
        );
    }

    #[test]
    fn auto_preference_preview_explains_fallback_when_umu_missing() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = crate::launch::test_support::ScopedCommandSearchPath::new(dir.path());

        let mut request = LaunchRequest {
            method: METHOD_PROTON_RUN.to_string(),
            game_path: "/tmp/game.exe".to_string(),
            umu_preference: crate::settings::UmuPreference::Auto,
            ..Default::default()
        };
        request.runtime.proton_path = "/opt/proton/GE-Proton9-20/proton".to_string();

        let preview = build_launch_preview(&request).unwrap();
        let decision = preview.umu_decision.as_ref().unwrap();
        assert!(!decision.will_use_umu, "Auto + no umu-run must not use umu");
        assert_eq!(
            decision.reason,
            "preference = Auto but umu-run was not found on the backend PATH — falling back to direct Proton"
        );
    }
}
