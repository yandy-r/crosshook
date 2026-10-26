use std::error::Error;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::args::{GlobalOptions, LaunchCommand};
use crate::cli_error::CliError;
use crate::store::profile_store;
use crosshook_app::log_tail::LogBatch;
use crosshook_app::services::launch::{read_log_tail, stream_child_log};
use crosshook_core::launch::diagnostics::MAX_LOG_TAIL_BYTES;
use crosshook_core::launch::{
    self, build_launch_preview, LaunchRequest, ValidationSeverity, METHOD_NATIVE,
    METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH,
};
use crosshook_core::metadata::MetadataStore;
use crosshook_core::profile::resolve_launch_method;
use crosshook_core::runtime_helpers::{self, RuntimeHelper};
use crosshook_core::settings::SettingsStore;
use crosshook_core::umu_database;

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
    let steam_client_install_path =
        resolve_steam_client_install_path(&profile.steam.compatdata_path);
    let mut request = LaunchRequest::from_profile(
        &profile,
        resolve_launch_method(&profile),
        &steam_client_install_path.to_string_lossy(),
        &profile_name,
        settings.umu_preference,
    )
    .map_err(|e| CliError::LaunchFailure(e.to_string()))?;
    request.launch_game_only = true;
    let metadata_store = MetadataStore::try_new().unwrap_or_else(|_| MetadataStore::disabled());
    request.resolved_umu_game_id = Some(
        umu_database::resolve_umu_game_id(&request, settings.umu_database_lookup, &metadata_store)
            .await,
    );
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

    let log_path = launch_log_path(&profile_name);
    let method = request.method.as_str();

    if let Some(parent) = log_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| CliError::LaunchFailure(format!("failed to create log directory: {e}")))?;
    }

    let mut child = match method {
        METHOD_STEAM_APPLAUNCH => {
            let helper = match command.scripts_dir.as_deref() {
                Some(dir) => {
                    let helper = dir.join(RuntimeHelper::SteamLaunchHelper.file_name());
                    validate_explicit_helper(&helper)?;
                    helper
                }
                // Already vetted by the core trust check.
                None => {
                    runtime_helpers::resolve(RuntimeHelper::SteamLaunchHelper).map_err(|e| {
                        CliError::LaunchFailure(format!(
                            "{e}; pass --scripts-dir <dir containing steam-launch-helper.sh>"
                        ))
                    })?
                }
            };
            spawn_helper(&request, &helper, &log_path)
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
    let mut output_error = None;
    let status = stream_child_log(&mut child, &log_path, |batch| {
        if let LogBatch::Lines(lines) = batch {
            if output_error.is_none() {
                let mut stdout = std::io::stdout().lock();
                output_error = lines
                    .iter()
                    .try_for_each(|line| writeln!(stdout, "{line}"))
                    .and_then(|()| stdout.flush())
                    .err();
            }
        }
    })
    .await
    .map_err(|e| CliError::LaunchFailure(e.to_string()))?;
    if let Some(error) = output_error {
        return Err(CliError::LaunchFailure(error.to_string()));
    }

    let log_tail = read_log_tail(&log_path, MAX_LOG_TAIL_BYTES).await;
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

/// C-1 mitigation for a user-supplied `--scripts-dir`: the helper must be a
/// regular file owned by the current user.
fn validate_explicit_helper(helper: &Path) -> Result<(), CliError> {
    let meta = std::fs::metadata(helper).map_err(|e| {
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
    Ok(())
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

fn spawn_helper(
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

fn severity_label(severity: ValidationSeverity) -> &'static str {
    match severity {
        ValidationSeverity::Fatal => "fatal",
        ValidationSeverity::Warning => "warning",
        ValidationSeverity::Info => "info",
    }
}
