use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::args::{GlobalOptions, LaunchCommand};
use crate::cli_error::CliError;
use crate::store::profile_store;
use crosshook_core::launch::request::LaunchOptimizationsRequest;
use crosshook_core::launch::{
    self, build_launch_preview, LaunchRequest, RuntimeLaunchConfig, SteamLaunchConfig,
    ValidationSeverity, METHOD_NATIVE, METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH,
};
use crosshook_core::profile::{resolve_launch_method, GameProfile};
use crosshook_core::settings::{SettingsStore, UmuPreference};
use tokio::fs::{self, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::time::{sleep, Duration};

const DEFAULT_SCRIPTS_DIR: &str = "../../runtime-helpers";
const HELPER_SCRIPT_NAME: &str = "steam-launch-helper.sh";

pub(crate) async fn launch_profile(
    command: LaunchCommand,
    global: &GlobalOptions,
) -> Result<(), CliError> {
    let profile_name = command
        .profile
        .or_else(|| global.profile.clone())
        .ok_or("a profile name is required via --profile")?;
    let store = profile_store(
        command
            .profile_dir
            .clone()
            .or_else(|| global.config.clone()),
    )?;
    let profile = store.load(&profile_name).map_err(|e| {
        CliError::ProfileNotFound(format!("profile \"{profile_name}\" not found: {e}"))
    })?;
    let settings_store =
        SettingsStore::try_new().map_err(|e| CliError::General(format!("settings store: {e}")))?;
    let settings = settings_store
        .load()
        .map_err(|e| CliError::General(format!("settings load: {e}")))?;
    let effective_umu_preference = profile
        .runtime
        .umu_preference
        .unwrap_or(settings.umu_preference);
    let request = launch_request_from_profile(&profile, &profile_name, effective_umu_preference)
        .map_err(|e| CliError::LaunchFailure(e.to_string()))?;
    launch::validate(&request).map_err(|e| CliError::LaunchFailure(e.to_string()))?;

    if command.dry_run {
        if global.json {
            let preview = build_launch_preview(&request)
                .map_err(|e| CliError::LaunchFailure(e.to_string()))?;
            println!("{}", serde_json::to_string_pretty(&preview)?);
        } else {
            let preview = build_launch_preview(&request)
                .map_err(|e| CliError::LaunchFailure(e.to_string()))?;
            println!("Method:     {}", request.method);
            println!("Game:       {}", request.game_path);
            if !request.trainer_path.is_empty() {
                println!("Trainer:    {}", request.trainer_path);
            }
            if let Some(ref cmd) = preview.effective_command {
                println!("Command:    {cmd}");
            }
            if let Some(ref env) = preview.environment {
                if !env.is_empty() {
                    println!("Environment:");
                    for var in env {
                        println!("  {}={}", var.key, var.value);
                    }
                }
            }
            if let Some(ref wrappers) = preview.wrappers {
                if !wrappers.is_empty() {
                    println!("Wrappers:   {}", wrappers.join(" → "));
                }
            }
            if let Some(ref opts) = preview.steam_launch_options {
                println!("Steam opts: {opts}");
            }
            if let Some(ref err) = preview.directives_error {
                eprintln!("warning: {err}");
            }
        }
        return Ok(());
    }

    let scripts_dir = command.scripts_dir.unwrap_or_else(default_scripts_dir);
    let log_path = launch_log_path(&profile_name);
    let method = request.method.as_str();

    if let Some(parent) = log_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| CliError::LaunchFailure(format!("failed to create log directory: {e}")))?;
    }

    let mut child = match method {
        METHOD_STEAM_APPLAUNCH => {
            let helper = scripts_dir.join(HELPER_SCRIPT_NAME);
            // C-1 mitigation: validate helper script before execution
            let meta = std::fs::metadata(&helper).map_err(|e| {
                CliError::LaunchFailure(format!(
                    "helper script not found at {}: {e}",
                    helper.display()
                ))
            })?;
            if !meta.is_file() {
                return Err(CliError::LaunchFailure(format!(
                    "helper script is not a regular file: {}",
                    helper.display()
                )));
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                let uid = unsafe { libc::getuid() };
                if meta.uid() != uid {
                    return Err(CliError::LaunchFailure(format!(
                        "helper script {} is not owned by current user (uid {})",
                        helper.display(),
                        uid
                    )));
                }
            }
            spawn_helper(&request, &helper, &log_path)
                .await
                .map_err(|e| CliError::LaunchFailure(e.to_string()))?
        }
        METHOD_PROTON_RUN => {
            let mut cmd = launch::script_runner::build_proton_game_command(&request, &log_path)
                .map_err(|e| CliError::LaunchFailure(e.to_string()))?;
            cmd.stdout(Stdio::null()).stderr(Stdio::null());
            cmd.spawn()
                .map_err(|e| CliError::LaunchFailure(e.to_string()))?
        }
        METHOD_NATIVE => {
            let mut cmd = launch::script_runner::build_native_game_command(&request, &log_path)
                .map_err(|e| CliError::LaunchFailure(e.to_string()))?;
            cmd.stdout(Stdio::null()).stderr(Stdio::null());
            cmd.spawn()
                .map_err(|e| CliError::LaunchFailure(e.to_string()))?
        }
        other => {
            return Err(CliError::LaunchFailure(format!(
                "unsupported launch method: {other}"
            )))
        }
    };
    let status = stream_helper_log(&mut child, &log_path)
        .await
        .map_err(|e| CliError::LaunchFailure(e.to_string()))?;

    let log_tail = safe_read_tail(
        &log_path,
        crosshook_core::launch::diagnostics::MAX_LOG_TAIL_BYTES,
    )
    .await;
    let report = launch::analyze(Some(status), &log_tail, &request.method);
    if launch::should_surface_report(&report) {
        eprintln!("{}", report.summary);
        for pattern_match in &report.pattern_matches {
            eprintln!(
                "[{}] {}: {}",
                severity_label(pattern_match.severity),
                pattern_match.summary,
                pattern_match.suggestion
            );
        }
    }
    if !status.success() {
        return Err(CliError::LaunchFailure(format!(
            "launch process exited with status {status}"
        )));
    }

    Ok(())
}

fn launch_request_from_profile(
    profile: &GameProfile,
    profile_name: &str,
    umu_preference: UmuPreference,
) -> Result<LaunchRequest, Box<dyn Error>> {
    let method = resolve_launch_method(profile);
    let steam_client_install_path =
        resolve_steam_client_install_path(&profile.steam.compatdata_path);

    Ok(LaunchRequest {
        method: method.to_string(),
        game_path: profile.game.executable_path.clone(),
        trainer_path: profile.trainer.path.clone(),
        trainer_host_path: profile.trainer.path.clone(),
        trainer_loading_mode: profile.trainer.loading_mode,
        steam: match method {
            METHOD_STEAM_APPLAUNCH => SteamLaunchConfig {
                app_id: profile.steam.app_id.clone(),
                compatdata_path: profile.steam.compatdata_path.clone(),
                proton_path: profile.steam.proton_path.clone(),
                steam_client_install_path: steam_client_install_path.to_string_lossy().into_owned(),
            },
            _ => SteamLaunchConfig {
                steam_client_install_path: steam_client_install_path.to_string_lossy().into_owned(),
                ..Default::default()
            },
        },
        runtime: match method {
            METHOD_PROTON_RUN => RuntimeLaunchConfig {
                prefix_path: profile.runtime.prefix_path.clone(),
                proton_path: profile.runtime.proton_path.clone(),
                working_directory: profile.runtime.working_directory.clone(),
                steam_app_id: profile.runtime.steam_app_id.clone(),
                umu_game_id: profile.runtime.umu_game_id.clone(),
            },
            METHOD_NATIVE => RuntimeLaunchConfig {
                working_directory: profile.runtime.working_directory.clone(),
                ..Default::default()
            },
            _ => RuntimeLaunchConfig::default(),
        },
        optimizations: LaunchOptimizationsRequest {
            enabled_option_ids: profile.launch.optimizations.enabled_option_ids.clone(),
        },
        launch_trainer_only: false,
        launch_game_only: true,
        profile_name: Some(profile_name.to_string()),
        custom_env_vars: profile.launch.custom_env_vars.clone(),
        umu_preference,
        network_isolation: profile.launch.network_isolation,
        gamescope: profile.launch.gamescope.clone(),
        trainer_gamescope: if profile.launch.trainer_gamescope.is_default() {
            None
        } else {
            Some(profile.launch.trainer_gamescope.clone())
        },
        mangohud: profile.launch.mangohud.clone(),
    })
}

fn resolve_steam_client_install_path(compatdata_path: &str) -> PathBuf {
    if let Ok(value) = std::env::var("STEAM_COMPAT_CLIENT_INSTALL_PATH") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    let compatdata = Path::new(compatdata_path);
    for ancestor in compatdata.ancestors() {
        let candidate = ancestor.join("steam.sh");
        if candidate.exists() {
            return ancestor.to_path_buf();
        }
    }

    default_steam_roots()
        .into_iter()
        .find(|candidate| candidate.join("steam.sh").exists())
        .unwrap_or_default()
}

fn default_steam_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Ok(home) = std::env::var("HOME") {
        let home = PathBuf::from(home);
        roots.push(home.join(".steam/root"));
        roots.push(home.join(".local/share/Steam"));
        roots.push(home.join(".var/app/com.valvesoftware.Steam/data/Steam"));
    }

    roots
}

fn default_scripts_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_SCRIPTS_DIR)
}

fn launch_log_dir() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("crosshook-logs")
}

fn launch_log_path(profile_name: &str) -> PathBuf {
    let safe_name = profile_name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();

    launch_log_dir().join(format!("{safe_name}.log"))
}

async fn spawn_helper(
    request: &LaunchRequest,
    helper_script: &Path,
    log_path: &Path,
) -> Result<tokio::process::Child, Box<dyn Error>> {
    let mut command =
        launch::script_runner::build_helper_command(request, helper_script, log_path)?;
    command.stdout(Stdio::null());
    command.stderr(Stdio::null());
    Ok(command.spawn()?)
}

async fn stream_helper_log(
    child: &mut tokio::process::Child,
    log_path: &Path,
) -> Result<std::process::ExitStatus, Box<dyn Error>> {
    let mut offset = 0u64;
    let mut stdout = tokio::io::stdout();

    loop {
        if let Some(status) = child.try_wait()? {
            let _ = drain_log(log_path, offset, &mut stdout).await?;
            stdout.flush().await?;
            return Ok(status);
        }

        offset = drain_log(log_path, offset, &mut stdout).await?;
        stdout.flush().await?;
        sleep(Duration::from_millis(500)).await;
    }
}

async fn drain_log(
    log_path: &Path,
    offset: u64,
    stdout: &mut tokio::io::Stdout,
) -> Result<u64, Box<dyn Error>> {
    let file = match OpenOptions::new().read(true).open(log_path).await {
        Ok(file) => file,
        Err(_) => return Ok(offset),
    };

    let mut file = file;
    file.seek(std::io::SeekFrom::Start(offset)).await?;

    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).await?;
    if !buffer.is_empty() {
        stdout.write_all(&buffer).await?;
        return Ok(offset + buffer.len() as u64);
    }

    Ok(offset)
}

async fn safe_read_tail(path: &Path, max_bytes: u64) -> String {
    let file = match fs::File::open(path).await {
        Ok(file) => file,
        Err(_) => return String::new(),
    };

    let metadata = match file.metadata().await {
        Ok(metadata) => metadata,
        Err(_) => return String::new(),
    };

    let mut file = file;
    if metadata.len() > max_bytes {
        let offset = -(max_bytes as i64);
        if file.seek(std::io::SeekFrom::End(offset)).await.is_err() {
            return String::new();
        }
    }

    let mut buffer = Vec::new();
    if file.read_to_end(&mut buffer).await.is_err() {
        return String::new();
    }

    String::from_utf8_lossy(&buffer).into_owned()
}

fn severity_label(severity: ValidationSeverity) -> &'static str {
    match severity {
        ValidationSeverity::Fatal => "fatal",
        ValidationSeverity::Warning => "warning",
        ValidationSeverity::Info => "info",
    }
}
