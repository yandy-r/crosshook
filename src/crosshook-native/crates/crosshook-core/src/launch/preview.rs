use std::path::Path;

use serde::Serialize;

use super::env::WINE_ENV_VARS_TO_CLEAR;
use super::optimizations::{
    build_steam_launch_options_command, resolve_launch_directives, LaunchDirectives,
};
use super::request::{
    validate_all, LaunchRequest, LaunchValidationIssue, METHOD_NATIVE, METHOD_PROTON_RUN,
    METHOD_STEAM_APPLAUNCH,
};
use super::runtime_helpers::{
    env_value, resolve_proton_paths, resolve_steam_client_install_path, DEFAULT_HOST_PATH,
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
}

impl LaunchPreview {
    /// Renders a human-readable TOML-like text summary for clipboard copy.
    pub fn to_display_toml(&self) -> String {
        let mut lines = Vec::new();

        // [preview]
        lines.push("[preview]".to_string());
        lines.push(format!("generated_at = \"{}\"", self.generated_at));
        lines.push(format!(
            "method = \"{}\"",
            self.resolved_method.as_str()
        ));
        lines.push(format!("game = \"{}\"", self.game_executable));
        lines.push(format!(
            "game_name = \"{}\"",
            self.game_executable_name
        ));
        if !self.working_directory.is_empty() {
            lines.push(format!(
                "working_directory = \"{}\"",
                self.working_directory
            ));
        }
        lines.push(String::new());

        // [validation]
        lines.push("[validation]".to_string());
        lines.push(format!(
            "passed = {}",
            self.validation.issues.is_empty()
        ));
        lines.push(format!("issue_count = {}", self.validation.issues.len()));
        for issue in &self.validation.issues {
            lines.push(format!(
                "  [{:?}] {}",
                issue.severity, issue.message
            ));
        }
        lines.push(String::new());

        // [command]
        lines.push("[command]".to_string());
        if let Some(ref cmd) = self.effective_command {
            lines.push(format!("effective = \"{}\"", cmd));
        }
        if let Some(ref opts) = self.steam_launch_options {
            lines.push(format!("steam_launch_options = \"{}\"", opts));
        }
        if let Some(ref wrappers) = self.wrappers {
            if !wrappers.is_empty() {
                lines.push(format!("wrappers = {:?}", wrappers));
            }
        }
        if let Some(ref err) = self.directives_error {
            lines.push(format!("error = \"{}\"", err));
        }
        lines.push(String::new());

        // [proton]
        if let Some(ref setup) = self.proton_setup {
            lines.push("[proton]".to_string());
            lines.push(format!(
                "proton_executable = \"{}\"",
                setup.proton_executable
            ));
            lines.push(format!(
                "wine_prefix_path = \"{}\"",
                setup.wine_prefix_path
            ));
            lines.push(format!(
                "compat_data_path = \"{}\"",
                setup.compat_data_path
            ));
            lines.push(format!(
                "steam_client_install_path = \"{}\"",
                setup.steam_client_install_path
            ));
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
                lines.push(format!("staged_path = \"{}\"", staged));
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
                lines.push(format!("  {}", var));
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

    // Resolve launch directives (wrappers + optimization env).
    // This can fail independently of validation (e.g., missing wrapper binary).
    // On failure, capture the error and continue with partial results.
    let (directives, mut directives_error) = match resolve_launch_directives(request) {
        Ok(d) => (Some(d), None),
        Err(e) => (None, Some(e.to_string())),
    };

    // Environment and command depend on successful directive resolution.
    let (environment, wrappers, effective_command) = match &directives {
        Some(directives) => {
            let mut env = Vec::new();
            collect_host_environment(&mut env);
            match resolved_method {
                ResolvedLaunchMethod::SteamApplaunch => {
                    collect_steam_proton_environment(request, &mut env);
                }
                ResolvedLaunchMethod::ProtonRun => {
                    collect_runtime_proton_environment(request, &mut env);
                    collect_optimization_environment(directives, &mut env);
                }
                ResolvedLaunchMethod::Native => {}
            }
            let effective_command =
                match build_effective_command_string(request, resolved_method, directives) {
                    Ok(command) => Some(command),
                    Err(error) => {
                        append_preview_error(&mut directives_error, error);
                        None
                    }
                };
            (Some(env), Some(directives.wrappers.clone()), effective_command)
        }
        None => (None, None, None),
    };

    // Steam launch options (for copy/paste)
    let steam_launch_options = if resolved_method == ResolvedLaunchMethod::SteamApplaunch {
        match build_steam_launch_options_command(&request.optimizations.enabled_option_ids) {
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
            .map(|s| s.to_string())
            .collect()
    } else {
        Vec::new()
    };
    let working_directory = resolve_working_directory(request, resolved_method.as_str());
    let generated_at = chrono::Utc::now().to_rfc3339();

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
    };

    preview.display_text = preview.to_display_toml();
    Ok(preview)
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
        value: resolved_paths.wine_prefix_path.to_string_lossy().into_owned(),
        source: EnvVarSource::ProtonRuntime,
    });

    env.push(PreviewEnvVar {
        key: "STEAM_COMPAT_DATA_PATH".to_string(),
        value: resolved_paths.compat_data_path.to_string_lossy().into_owned(),
        source: EnvVarSource::ProtonRuntime,
    });

    if let Some(steam_client_path) = resolve_steam_client_install_path(
        request.steam.steam_client_install_path.trim(),
    ) {
        env.push(PreviewEnvVar {
            key: "STEAM_COMPAT_CLIENT_INSTALL_PATH".to_string(),
            value: steam_client_path,
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

/// Maps resolved launch directive environment pairs to preview env vars.
fn collect_optimization_environment(
    directives: &LaunchDirectives,
    env: &mut Vec<PreviewEnvVar>,
) {
    for (key, value) in &directives.env {
        env.push(PreviewEnvVar {
            key: key.clone(),
            value: value.clone(),
            source: EnvVarSource::LaunchOptimization,
        });
    }
}

/// Builds a human-readable command string showing the effective launch command.
fn build_effective_command_string(
    request: &LaunchRequest,
    method: ResolvedLaunchMethod,
    directives: &LaunchDirectives,
) -> Result<String, String> {
    match method {
        ResolvedLaunchMethod::ProtonRun => {
            let mut parts: Vec<String> = Vec::new();
            for wrapper in &directives.wrappers {
                parts.push(wrapper.clone());
            }
            parts.push(request.runtime.proton_path.trim().to_string());
            parts.push("run".to_string());
            parts.push(request.game_path.trim().to_string());
            Ok(parts.join(" "))
        }
        ResolvedLaunchMethod::SteamApplaunch => {
            build_steam_launch_options_command(&request.optimizations.enabled_option_ids)
                .map_err(|error| error.to_string())
        }
        ResolvedLaunchMethod::Native => Ok(request.game_path.trim().to_string()),
    }
}

/// Builds Proton setup details. Returns `None` for native method.
fn build_proton_setup(request: &LaunchRequest, method: &str) -> Option<ProtonSetup> {
    match method {
        METHOD_PROTON_RUN => {
            let resolved_paths = resolve_proton_paths(Path::new(request.runtime.prefix_path.trim()));

            let steam_client = resolve_steam_client_install_path(
                request.steam.steam_client_install_path.trim(),
            )
            .unwrap_or_default();

            Some(ProtonSetup {
                wine_prefix_path: resolved_paths.wine_prefix_path.to_string_lossy().into_owned(),
                compat_data_path: resolved_paths.compat_data_path.to_string_lossy().into_owned(),
                steam_client_install_path: steam_client,
                proton_executable: request.runtime.proton_path.trim().to_string(),
            })
        }
        METHOD_STEAM_APPLAUNCH => {
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
                "C:\\CrossHook\\StagedTrainers\\{}\\{}",
                file_stem, file_name
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
    use serde_json::json;
    use crate::launch::request::{
        LaunchOptimizationsRequest, RuntimeLaunchConfig, SteamLaunchConfig, METHOD_NATIVE,
        METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH,
    };
    use crate::profile::TrainerLoadingMode;

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
        assert_eq!(
            preview.steam_launch_options.as_deref(),
            Some("%command%")
        );
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
}
