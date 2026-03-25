use std::error::Error;
use std::fmt;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::optimizations::resolve_launch_directives;

pub const METHOD_STEAM_APPLAUNCH: &str = "steam_applaunch";
pub const METHOD_PROTON_RUN: &str = "proton_run";
pub const METHOD_NATIVE: &str = "native";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LaunchRequest {
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub game_path: String,
    #[serde(default)]
    pub trainer_path: String,
    #[serde(default)]
    pub trainer_host_path: String,
    #[serde(default)]
    pub steam: SteamLaunchConfig,
    #[serde(default)]
    pub runtime: RuntimeLaunchConfig,
    #[serde(default)]
    pub optimizations: LaunchOptimizationsRequest,
    #[serde(default)]
    pub launch_trainer_only: bool,
    #[serde(default)]
    pub launch_game_only: bool,
}

pub type SteamLaunchRequest = LaunchRequest;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SteamLaunchConfig {
    #[serde(default)]
    pub app_id: String,
    #[serde(default)]
    pub compatdata_path: String,
    #[serde(default)]
    pub proton_path: String,
    #[serde(default)]
    pub steam_client_install_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RuntimeLaunchConfig {
    #[serde(default)]
    pub prefix_path: String,
    #[serde(default)]
    pub proton_path: String,
    #[serde(default)]
    pub working_directory: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LaunchOptimizationsRequest {
    #[serde(
        rename = "enabled_option_ids",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub enabled_option_ids: Vec<String>,
}

impl LaunchRequest {
    pub fn resolved_method(&self) -> &str {
        match self.method.trim() {
            METHOD_STEAM_APPLAUNCH => METHOD_STEAM_APPLAUNCH,
            METHOD_PROTON_RUN => METHOD_PROTON_RUN,
            METHOD_NATIVE => METHOD_NATIVE,
            _ if !self.steam.app_id.trim().is_empty() => METHOD_STEAM_APPLAUNCH,
            _ if looks_like_windows_executable(&self.game_path) => METHOD_PROTON_RUN,
            _ => METHOD_NATIVE,
        }
    }

    pub fn game_executable_name(&self) -> String {
        let trimmed_path = self.game_path.trim();

        if trimmed_path.is_empty() {
            return String::new();
        }

        let separator_index = trimmed_path
            .char_indices()
            .rev()
            .find_map(|(index, character)| matches!(character, '/' | '\\').then_some(index));

        match separator_index {
            Some(index) if index + 1 < trimmed_path.len() => trimmed_path[index + 1..].to_string(),
            Some(_) => String::new(),
            None => trimmed_path.to_string(),
        }
    }

    pub fn log_target_slug(&self) -> String {
        let game_executable_name = self.game_executable_name();
        let source = match self.resolved_method() {
            METHOD_STEAM_APPLAUNCH => self.steam.app_id.trim(),
            _ => game_executable_name.trim(),
        };

        let fallback = match self.resolved_method() {
            METHOD_STEAM_APPLAUNCH => "steam",
            METHOD_PROTON_RUN => "proton",
            METHOD_NATIVE => "native",
            _ => "launch",
        };

        let candidate = if source.is_empty() { fallback } else { source };
        let slug = candidate
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() {
                    character.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect::<String>();

        let trimmed = slug.trim_matches('-');
        if trimmed.is_empty() {
            fallback.to_string()
        } else {
            trimmed.to_string()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    GamePathRequired,
    GamePathMissing,
    GamePathNotFile,
    TrainerPathRequired,
    TrainerHostPathRequired,
    TrainerHostPathMissing,
    TrainerHostPathNotFile,
    SteamAppIdRequired,
    SteamCompatDataPathRequired,
    SteamCompatDataPathMissing,
    SteamCompatDataPathNotDirectory,
    SteamProtonPathRequired,
    SteamProtonPathMissing,
    SteamProtonPathNotExecutable,
    SteamClientInstallPathRequired,
    RuntimePrefixPathRequired,
    RuntimePrefixPathMissing,
    RuntimePrefixPathNotDirectory,
    RuntimeProtonPathRequired,
    RuntimeProtonPathMissing,
    RuntimeProtonPathNotExecutable,
    UnknownLaunchOptimization(String),
    DuplicateLaunchOptimization(String),
    LaunchOptimizationsUnsupportedForMethod(String),
    LaunchOptimizationNotSupportedForMethod { option_id: String, method: String },
    IncompatibleLaunchOptimizations { first: String, second: String },
    LaunchOptimizationDependencyMissing { option_id: String, dependency: String },
    NativeWindowsExecutableNotSupported,
    NativeTrainerLaunchUnsupported,
    UnsupportedMethod(String),
}

impl ValidationError {
    pub fn message(&self) -> String {
        match self {
            Self::GamePathRequired => "A game executable path is required.".to_string(),
            Self::GamePathMissing => {
                "The selected game executable path does not exist.".to_string()
            }
            Self::GamePathNotFile => {
                "The selected game executable path must be a file.".to_string()
            }
            Self::TrainerPathRequired => {
                "A trainer path is required for trainer launch.".to_string()
            }
            Self::TrainerHostPathRequired => {
                "A trainer host path is required for trainer launch.".to_string()
            }
            Self::TrainerHostPathMissing => "The trainer host path does not exist.".to_string(),
            Self::TrainerHostPathNotFile => "The trainer host path must be a file.".to_string(),
            Self::SteamAppIdRequired => "Steam app launch requires a Steam App ID.".to_string(),
            Self::SteamCompatDataPathRequired => {
                "Steam app launch requires a compatdata path.".to_string()
            }
            Self::SteamCompatDataPathMissing => {
                "The Steam compatdata path does not exist.".to_string()
            }
            Self::SteamCompatDataPathNotDirectory => {
                "The Steam compatdata path must be a directory.".to_string()
            }
            Self::SteamProtonPathRequired => "Steam app launch requires a Proton path.".to_string(),
            Self::SteamProtonPathMissing => "The Steam Proton path does not exist.".to_string(),
            Self::SteamProtonPathNotExecutable => {
                "The Steam Proton path must be executable.".to_string()
            }
            Self::SteamClientInstallPathRequired => {
                "Steam app launch requires a Steam client install path.".to_string()
            }
            Self::RuntimePrefixPathRequired => {
                "Proton launch requires a runtime prefix path.".to_string()
            }
            Self::RuntimePrefixPathMissing => "The runtime prefix path does not exist.".to_string(),
            Self::RuntimePrefixPathNotDirectory => {
                "The runtime prefix path must be a directory.".to_string()
            }
            Self::RuntimeProtonPathRequired => {
                "Proton launch requires a runtime Proton path.".to_string()
            }
            Self::RuntimeProtonPathMissing => "The runtime Proton path does not exist.".to_string(),
            Self::RuntimeProtonPathNotExecutable => {
                "The runtime Proton path must be executable.".to_string()
            }
            Self::UnknownLaunchOptimization(option_id) => {
                format!("Unknown launch optimization '{option_id}'.")
            }
            Self::DuplicateLaunchOptimization(option_id) => {
                format!("Launch optimization '{option_id}' was selected more than once.")
            }
            Self::LaunchOptimizationsUnsupportedForMethod(method) => {
                format!("Launch optimizations are only supported for proton_run launches, not '{method}'.")
            }
            Self::LaunchOptimizationNotSupportedForMethod { option_id, method } => {
                format!("Launch optimization '{option_id}' is not supported for '{method}' launches.")
            }
            Self::IncompatibleLaunchOptimizations { first, second } => {
                format!("Launch optimizations '{first}' and '{second}' cannot be enabled together.")
            }
            Self::LaunchOptimizationDependencyMissing {
                option_id,
                dependency,
            } => {
                format!("Launch optimization '{option_id}' requires '{dependency}' to be installed and available on PATH.")
            }
            Self::NativeWindowsExecutableNotSupported => {
                "Native launch only supports Linux-native executables, not Windows .exe files."
                    .to_string()
            }
            Self::NativeTrainerLaunchUnsupported => {
                "Native launch does not support the two-step trainer launch workflow.".to_string()
            }
            Self::UnsupportedMethod(method) => {
                format!(
                    "Unsupported launch method '{method}'. Use steam_applaunch, proton_run, or native."
                )
            }
        }
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message())
    }
}

impl Error for ValidationError {}

pub fn validate(request: &LaunchRequest) -> Result<(), ValidationError> {
    match request.method.trim() {
        "" | METHOD_STEAM_APPLAUNCH | METHOD_PROTON_RUN | METHOD_NATIVE => {}
        value => return Err(ValidationError::UnsupportedMethod(value.to_string())),
    }

    match request.resolved_method() {
        METHOD_STEAM_APPLAUNCH => validate_steam_applaunch(request),
        METHOD_PROTON_RUN => validate_proton_run(request),
        METHOD_NATIVE => validate_native(request),
        other => Err(ValidationError::UnsupportedMethod(other.to_string())),
    }
}

fn validate_steam_applaunch(request: &LaunchRequest) -> Result<(), ValidationError> {
    require_game_path_if_needed(request, false)?;
    require_trainer_paths_if_needed(request)?;

    if request.steam.app_id.trim().is_empty() {
        return Err(ValidationError::SteamAppIdRequired);
    }

    require_directory(
        request.steam.compatdata_path.trim(),
        ValidationError::SteamCompatDataPathRequired,
        ValidationError::SteamCompatDataPathMissing,
        ValidationError::SteamCompatDataPathNotDirectory,
    )?;

    require_executable_file(
        request.steam.proton_path.trim(),
        ValidationError::SteamProtonPathRequired,
        ValidationError::SteamProtonPathMissing,
        ValidationError::SteamProtonPathNotExecutable,
    )?;

    if request.steam.steam_client_install_path.trim().is_empty() {
        return Err(ValidationError::SteamClientInstallPathRequired);
    }

    reject_launch_optimizations_for_method(request, METHOD_STEAM_APPLAUNCH)?;

    Ok(())
}

fn validate_proton_run(request: &LaunchRequest) -> Result<(), ValidationError> {
    require_game_path_if_needed(request, true)?;
    require_trainer_paths_if_needed(request)?;

    require_directory(
        request.runtime.prefix_path.trim(),
        ValidationError::RuntimePrefixPathRequired,
        ValidationError::RuntimePrefixPathMissing,
        ValidationError::RuntimePrefixPathNotDirectory,
    )?;

    require_executable_file(
        request.runtime.proton_path.trim(),
        ValidationError::RuntimeProtonPathRequired,
        ValidationError::RuntimeProtonPathMissing,
        ValidationError::RuntimeProtonPathNotExecutable,
    )?;

    resolve_launch_directives(request)?;

    Ok(())
}

fn validate_native(request: &LaunchRequest) -> Result<(), ValidationError> {
    if request.launch_trainer_only {
        return Err(ValidationError::NativeTrainerLaunchUnsupported);
    }

    require_game_path_if_needed(request, true)?;

    if looks_like_windows_executable(&request.game_path) {
        return Err(ValidationError::NativeWindowsExecutableNotSupported);
    }

    reject_launch_optimizations_for_method(request, METHOD_NATIVE)?;

    Ok(())
}

fn reject_launch_optimizations_for_method(
    request: &LaunchRequest,
    method: &str,
) -> Result<(), ValidationError> {
    if request.optimizations.enabled_option_ids.is_empty() {
        return Ok(());
    }

    Err(ValidationError::LaunchOptimizationsUnsupportedForMethod(
        method.to_string(),
    ))
}

fn require_game_path_if_needed(
    request: &LaunchRequest,
    must_exist: bool,
) -> Result<(), ValidationError> {
    if request.launch_trainer_only {
        return Ok(());
    }

    let game_path = request.game_path.trim();
    if game_path.is_empty() {
        return Err(ValidationError::GamePathRequired);
    }

    if must_exist {
        let path = Path::new(game_path);
        if !path.exists() {
            return Err(ValidationError::GamePathMissing);
        }
        if !path.is_file() {
            return Err(ValidationError::GamePathNotFile);
        }
    }

    Ok(())
}

fn require_trainer_paths_if_needed(request: &LaunchRequest) -> Result<(), ValidationError> {
    if request.launch_game_only {
        return Ok(());
    }

    if request.trainer_path.trim().is_empty() {
        return Err(ValidationError::TrainerPathRequired);
    }

    let trainer_host_path = request.trainer_host_path.trim();
    if trainer_host_path.is_empty() {
        return Err(ValidationError::TrainerHostPathRequired);
    }

    let trainer_host = Path::new(trainer_host_path);
    if !trainer_host.exists() {
        return Err(ValidationError::TrainerHostPathMissing);
    }
    if !trainer_host.is_file() {
        return Err(ValidationError::TrainerHostPathNotFile);
    }

    Ok(())
}

fn require_directory<'a>(
    value: &'a str,
    required_error: ValidationError,
    missing_error: ValidationError,
    not_directory_error: ValidationError,
) -> Result<&'a Path, ValidationError> {
    if value.is_empty() {
        return Err(required_error);
    }

    let path = Path::new(value);
    if !path.exists() {
        return Err(missing_error);
    }
    if !path.is_dir() {
        return Err(not_directory_error);
    }

    Ok(path)
}

fn require_executable_file(
    value: &str,
    required_error: ValidationError,
    missing_error: ValidationError,
    not_executable_error: ValidationError,
) -> Result<(), ValidationError> {
    if value.is_empty() {
        return Err(required_error);
    }

    let path = Path::new(value);
    if !path.exists() {
        return Err(missing_error);
    }
    if !is_executable_file(path) {
        return Err(not_executable_error);
    }

    Ok(())
}

fn is_executable_file(path: &Path) -> bool {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => return false,
    };

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
    }

    #[cfg(not(unix))]
    {
        metadata.is_file()
    }
}

fn looks_like_windows_executable(path: &str) -> bool {
    path.trim().to_ascii_lowercase().ends_with(".exe")
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn validates_steam_applaunch_request() {
        let (_temp_dir, request) = steam_request();
        assert_eq!(validate(&request), Ok(()));
    }

    #[test]
    fn allows_game_only_steam_launch_without_trainer_paths() {
        let (_temp_dir, mut request) = steam_request();
        request.launch_game_only = true;
        request.trainer_path.clear();
        request.trainer_host_path.clear();

        assert_eq!(validate(&request), Ok(()));
    }

    #[test]
    fn validates_proton_run_request() {
        let (_temp_dir, request) = proton_request();
        assert_eq!(validate(&request), Ok(()));
    }

    #[test]
    fn proton_run_rejects_unknown_launch_optimization() {
        let (_temp_dir, mut request) = proton_request();
        request.optimizations.enabled_option_ids = vec!["unknown_toggle".to_string()];

        assert_eq!(
            validate(&request),
            Err(ValidationError::UnknownLaunchOptimization(
                "unknown_toggle".to_string()
            ))
        );
    }

    #[test]
    fn proton_run_rejects_duplicate_launch_optimizations() {
        let (_temp_dir, mut request) = proton_request();
        request.optimizations.enabled_option_ids = vec![
            "disable_steam_input".to_string(),
            "disable_steam_input".to_string(),
        ];

        assert_eq!(
            validate(&request),
            Err(ValidationError::DuplicateLaunchOptimization(
                "disable_steam_input".to_string()
            ))
        );
    }

    #[test]
    fn proton_run_rejects_conflicting_launch_optimizations() {
        let (_temp_dir, mut request) = proton_request();
        request.optimizations.enabled_option_ids = vec![
            "use_gamemode".to_string(),
            "use_game_performance".to_string(),
        ];

        assert_eq!(
            validate(&request),
            Err(ValidationError::IncompatibleLaunchOptimizations {
                first: "use_gamemode".to_string(),
                second: "use_game_performance".to_string(),
            })
        );
    }

    #[test]
    fn proton_run_requires_runtime_prefix_path() {
        let (_temp_dir, mut request) = proton_request();
        request.runtime.prefix_path.clear();

        assert_eq!(
            validate(&request),
            Err(ValidationError::RuntimePrefixPathRequired)
        );
    }

    #[test]
    fn native_requires_linux_native_executable() {
        let (_temp_dir, mut request) = native_request();
        request.game_path = request.game_path.replace("game.sh", "game.exe");
        fs::write(&request.game_path, b"game").expect("game exe");

        assert_eq!(
            validate(&request),
            Err(ValidationError::NativeWindowsExecutableNotSupported)
        );
    }

    #[test]
    fn native_rejects_trainer_only_launches() {
        let (_temp_dir, mut request) = native_request();
        request.launch_trainer_only = true;

        assert_eq!(
            validate(&request),
            Err(ValidationError::NativeTrainerLaunchUnsupported)
        );
    }

    #[test]
    fn native_rejects_launch_optimizations() {
        let (_temp_dir, mut request) = native_request();
        request.optimizations.enabled_option_ids = vec!["disable_steam_input".to_string()];

        assert_eq!(
            validate(&request),
            Err(ValidationError::LaunchOptimizationsUnsupportedForMethod(
                METHOD_NATIVE.to_string()
            ))
        );
    }

    #[test]
    fn rejects_unsupported_method() {
        let (_temp_dir, mut request) = steam_request();
        request.method = "direct".to_string();

        assert_eq!(
            validate(&request),
            Err(ValidationError::UnsupportedMethod("direct".to_string()))
        );
    }

    #[test]
    fn request_uses_last_path_segment_for_executable_name() {
        let request = LaunchRequest {
            game_path: r"Z:\Games\Test Game\game.exe".to_string(),
            optimizations: LaunchOptimizationsRequest::default(),
            ..LaunchRequest::default()
        };

        assert_eq!(request.game_executable_name(), "game.exe");
    }

    #[test]
    fn log_target_slug_prefers_game_name_for_non_steam_methods() {
        let request = LaunchRequest {
            method: METHOD_PROTON_RUN.to_string(),
            game_path: "/games/Example Game/game.exe".to_string(),
            optimizations: LaunchOptimizationsRequest::default(),
            ..LaunchRequest::default()
        };

        assert_eq!(request.log_target_slug(), "game-exe");
    }
}
