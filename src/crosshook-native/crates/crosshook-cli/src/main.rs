mod args;

use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use args::{
    Cli, Command, DiagnosticsArgs, DiagnosticsCommand, GlobalOptions, LaunchCommand, ProfileArgs,
    ProfileCommand, SteamArgs, SteamCommand,
};
use clap::{CommandFactory, Parser};
use crosshook_core::export::diagnostics::DiagnosticBundleOptions;
use crosshook_core::launch::request::LaunchOptimizationsRequest;
use crosshook_core::launch::{
    self, build_launch_preview, LaunchRequest, RuntimeLaunchConfig, SteamLaunchConfig,
    ValidationSeverity, METHOD_NATIVE, METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH,
};
use crosshook_core::profile::{
    export_community_profile, resolve_launch_method, GameProfile, ProfileStore,
};
use crosshook_core::settings::SettingsStore;
use crosshook_core::steam::discovery::discover_steam_root_candidates;
use crosshook_core::steam::libraries::discover_steam_libraries;
use crosshook_core::steam::proton::discover_compat_tools;
use crosshook_core::steam::{
    attempt_auto_populate, SteamAutoPopulateFieldState, SteamAutoPopulateRequest,
};
use tokio::fs::{self, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::time::{sleep, Duration};

const DEFAULT_SCRIPTS_DIR: &str = "../../runtime-helpers";
const HELPER_SCRIPT_NAME: &str = "steam-launch-helper.sh";

// Exit code constants — EXIT_USAGE_ERROR is handled by clap directly;
// EXIT_SUCCESS and EXIT_STEAM_NOT_FOUND are reserved for future use.
#[allow(dead_code)]
const EXIT_SUCCESS: i32 = 0;
const EXIT_GENERAL_ERROR: i32 = 1;
#[allow(dead_code)]
const EXIT_USAGE_ERROR: i32 = 2;
const EXIT_PROFILE_NOT_FOUND: i32 = 3;
const EXIT_LAUNCH_FAILURE: i32 = 4;
#[allow(dead_code)]
const EXIT_STEAM_NOT_FOUND: i32 = 5;

enum CliError {
    ProfileNotFound(String),
    LaunchFailure(String),
    General(String),
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProfileNotFound(msg) => write!(f, "{msg}"),
            Self::LaunchFailure(msg) => write!(f, "{msg}"),
            Self::General(msg) => write!(f, "{msg}"),
        }
    }
}

impl CliError {
    fn exit_code(&self) -> i32 {
        match self {
            Self::ProfileNotFound(_) => EXIT_PROFILE_NOT_FOUND,
            Self::LaunchFailure(_) => EXIT_LAUNCH_FAILURE,
            Self::General(_) => EXIT_GENERAL_ERROR,
        }
    }
}

impl From<Box<dyn Error>> for CliError {
    fn from(error: Box<dyn Error>) -> Self {
        Self::General(error.to_string())
    }
}

impl From<String> for CliError {
    fn from(msg: String) -> Self {
        Self::General(msg)
    }
}

impl From<&str> for CliError {
    fn from(msg: &str) -> Self {
        Self::General(msg.to_string())
    }
}

impl From<serde_json::Error> for CliError {
    fn from(error: serde_json::Error) -> Self {
        Self::General(error.to_string())
    }
}

impl From<crosshook_core::export::diagnostics::DiagnosticBundleError> for CliError {
    fn from(error: crosshook_core::export::diagnostics::DiagnosticBundleError) -> Self {
        Self::General(error.to_string())
    }
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("{error}");
        std::process::exit(error.exit_code());
    }
}

async fn run() -> Result<(), CliError> {
    let cli = Cli::parse();

    match cli.command {
        Command::Launch(command) => launch_profile(command, &cli.global).await?,
        Command::Profile(command) => handle_profile_command(command, &cli.global).await?,
        Command::Steam(command) => handle_steam_command(command, &cli.global).await?,
        Command::Diagnostics(args) => handle_diagnostics_command(args, &cli.global)?,
        Command::Status => handle_status_command(&cli.global).await?,
        Command::Completions { shell } => {
            let mut cmd = Cli::command();
            clap_complete::generate(shell, &mut cmd, "crosshook", &mut std::io::stdout());
        }
    }

    Ok(())
}

async fn launch_profile(command: LaunchCommand, global: &GlobalOptions) -> Result<(), CliError> {
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
    let request = launch_request_from_profile(&profile, &profile_name)
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

    // Ensure log directory exists before spawning
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

async fn handle_profile_command(
    command: ProfileArgs,
    global: &GlobalOptions,
) -> Result<(), CliError> {
    match command.command {
        ProfileCommand::List => {
            let store = profile_store(global.config.clone())?;
            let profiles = store
                .list()
                .map_err(|e| format!("failed to list profiles: {e}"))?;
            let count = profiles.len();
            let profiles_dir = store.base_path.to_string_lossy().into_owned();

            if global.json {
                #[derive(serde::Serialize)]
                struct ListOutput<'a> {
                    profiles: &'a [String],
                    count: usize,
                    profiles_dir: String,
                }
                let output = ListOutput {
                    profiles: &profiles,
                    count,
                    profiles_dir,
                };
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                for name in &profiles {
                    println!("{name}");
                }
                println!("{count} profile(s) in {profiles_dir}");
            }
        }
        ProfileCommand::Import(command) => {
            if global.verbose {
                eprintln!("legacy profile: {}", command.legacy_path.display());
            }

            // C-2 security mitigation: reject symlinks and non-files before opening
            let meta = std::fs::symlink_metadata(&command.legacy_path)
                .map_err(|e| format!("cannot access import path: {e}"))?;
            if !meta.file_type().is_file() {
                return Err(
                    "import path must be a regular file, not a symlink or directory".into(),
                );
            }

            let store = profile_store(global.config.clone())?;
            let profile = store
                .import_legacy(&command.legacy_path)
                .map_err(|e| format!("failed to import profile: {e}"))?;

            let profile_name = command
                .legacy_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");
            let launch_method = resolve_launch_method(&profile);

            if global.json {
                let output = serde_json::json!({
                    "imported": true,
                    "profile_name": profile_name,
                    "legacy_path": command.legacy_path.display().to_string(),
                    "launch_method": launch_method,
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                println!(
                    "Imported profile \"{}\" from {} (launch method: {})",
                    profile_name,
                    command.legacy_path.display(),
                    launch_method
                );
            }
        }
        ProfileCommand::Export(command) => {
            let profile_name = command
                .profile
                .or_else(|| global.profile.clone())
                .ok_or("a profile name is required; use --profile or -p")?;

            let output_path = command.output.unwrap_or_else(|| {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join(format!("{profile_name}.crosshook.json"))
            });

            // W-4 security mitigation: reject symlinks at output path
            if output_path
                .symlink_metadata()
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false)
            {
                return Err("output path is a symlink; refusing to write".into());
            }

            if let Some(parent) = output_path.parent() {
                if !parent.as_os_str().is_empty() && !parent.exists() {
                    return Err(
                        format!("output directory does not exist: {}", parent.display()).into(),
                    );
                }
            }

            let store = profile_store(global.config.clone())?;
            export_community_profile(store.base_path.as_path(), &profile_name, &output_path)
                .map_err(|e| format!("failed to export profile: {e}"))?;

            if global.json {
                let output = serde_json::json!({
                    "exported": true,
                    "profile_name": profile_name,
                    "output_path": output_path.display().to_string(),
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                println!(
                    "Exported profile \"{}\" to {}",
                    profile_name,
                    output_path.display()
                );
            }
        }
    }

    Ok(())
}

async fn handle_steam_command(command: SteamArgs, global: &GlobalOptions) -> Result<(), CliError> {
    match command.command {
        SteamCommand::Discover => {
            let mut diagnostics: Vec<String> = Vec::new();
            let roots = discover_steam_root_candidates("", &mut diagnostics);
            let libraries = discover_steam_libraries(&roots, &mut diagnostics);
            let proton_installs = discover_compat_tools(&roots, &mut diagnostics);

            if global.verbose {
                for msg in &diagnostics {
                    eprintln!("{msg}");
                }
            }

            if global.json {
                let output = serde_json::json!({
                    "roots": roots.iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>(),
                    "libraries": libraries,
                    "proton_installs": proton_installs,
                    "diagnostics": diagnostics,
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                println!("Steam roots: {}", roots.len());
                for root in &roots {
                    println!("  {}", root.display());
                }
                println!();
                println!("Libraries: {}", libraries.len());
                for lib in &libraries {
                    println!(
                        "  {} (steamapps: {})",
                        lib.path.display(),
                        lib.steamapps_path.display()
                    );
                }
                println!();
                println!("Proton installs: {}", proton_installs.len());
                for install in &proton_installs {
                    println!("  {} ({})", install.name, install.path.display());
                }
            }
        }
        SteamCommand::AutoPopulate(command) => {
            if global.verbose {
                eprintln!("game path: {}", command.game_path.display());
            }

            let request = SteamAutoPopulateRequest {
                game_path: command.game_path.clone(),
                steam_client_install_path: PathBuf::new(),
            };

            let result = attempt_auto_populate(&request);

            if global.verbose {
                for msg in &result.diagnostics {
                    eprintln!("{msg}");
                }
            }

            if global.json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!(
                    "App ID:      {} ({})",
                    result.app_id,
                    format_field_state(result.app_id_state)
                );
                println!(
                    "Compat Data: {} ({})",
                    result.compatdata_path.display(),
                    format_field_state(result.compatdata_state)
                );
                println!(
                    "Proton:      {} ({})",
                    result.proton_path.display(),
                    format_field_state(result.proton_state)
                );

                if !result.manual_hints.is_empty() {
                    println!();
                    for hint in &result.manual_hints {
                        println!("  hint: {hint}");
                    }
                }
            }
        }
    }

    Ok(())
}

fn handle_diagnostics_command(
    args: DiagnosticsArgs,
    global: &GlobalOptions,
) -> Result<(), CliError> {
    match args.command {
        DiagnosticsCommand::Export(command) => {
            let profile_store = profile_store(global.config.clone())?;
            let settings_store =
                SettingsStore::try_new().map_err(|error| format!("settings store: {error}"))?;

            let options = DiagnosticBundleOptions {
                redact_paths: command.redact_paths,
                output_dir: command.output,
            };

            let result = crosshook_core::export::export_diagnostic_bundle(
                &profile_store,
                &settings_store,
                &options,
            )?;

            if global.json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("Diagnostic bundle exported: {}", result.archive_path);
                println!("  Profiles:        {}", result.summary.profile_count);
                println!("  Log files:       {}", result.summary.log_file_count);
                println!("  Proton versions: {}", result.summary.proton_install_count);
            }

            Ok(())
        }
    }
}

async fn handle_status_command(global: &GlobalOptions) -> Result<(), CliError> {
    let mut diagnostics: Vec<String> = Vec::new();

    // Profiles
    let (profile_names, profiles_dir) = match profile_store(None) {
        Err(error) => {
            diagnostics.push(format!("profile store: {error}"));
            (Vec::new(), None)
        }
        Ok(store) => {
            let dir = Some(store.base_path.to_string_lossy().into_owned());
            let names = match store.list() {
                Ok(names) => names,
                Err(error) => {
                    diagnostics.push(format!("profile list: {error}"));
                    Vec::new()
                }
            };
            (names, dir)
        }
    };

    // Settings
    let settings_data = match SettingsStore::try_new() {
        Err(error) => {
            diagnostics.push(format!("settings store: {error}"));
            None
        }
        Ok(store) => match store.load() {
            Ok(data) => Some(data),
            Err(error) => {
                diagnostics.push(format!("settings load: {error}"));
                None
            }
        },
    };

    // Steam roots
    let steam_roots = discover_steam_root_candidates("", &mut diagnostics);

    // Proton installs
    let proton_installs = discover_compat_tools(&steam_roots, &mut diagnostics);

    if global.verbose {
        for diagnostic in &diagnostics {
            eprintln!("[status] {diagnostic}");
        }
    }

    let version = env!("CARGO_PKG_VERSION");

    if global.json {
        #[derive(serde::Serialize)]
        struct ProtonInfo {
            display_name: String,
            proton_path: String,
        }

        #[derive(serde::Serialize)]
        struct SteamInfo {
            roots: Vec<String>,
            proton_installs: Vec<ProtonInfo>,
        }

        #[derive(serde::Serialize)]
        struct ProfilesInfo {
            count: usize,
            names: Vec<String>,
            profiles_dir: Option<String>,
        }

        #[derive(serde::Serialize)]
        struct SettingsInfo {
            auto_load_last_profile: bool,
            last_used_profile: String,
            community_tap_count: usize,
            onboarding_completed: bool,
        }

        #[derive(serde::Serialize)]
        struct StatusOutput {
            version: String,
            profiles: ProfilesInfo,
            steam: SteamInfo,
            settings: Option<SettingsInfo>,
            diagnostics: Vec<String>,
        }

        let output = StatusOutput {
            version: version.to_string(),
            profiles: ProfilesInfo {
                count: profile_names.len(),
                names: profile_names,
                profiles_dir,
            },
            steam: SteamInfo {
                roots: steam_roots
                    .iter()
                    .map(|p| p.to_string_lossy().into_owned())
                    .collect(),
                proton_installs: proton_installs
                    .iter()
                    .map(|p| ProtonInfo {
                        display_name: p.name.clone(),
                        proton_path: p.path.to_string_lossy().into_owned(),
                    })
                    .collect(),
            },
            settings: settings_data.map(|s| SettingsInfo {
                auto_load_last_profile: s.auto_load_last_profile,
                last_used_profile: s.last_used_profile,
                community_tap_count: s.community_taps.len(),
                onboarding_completed: s.onboarding_completed,
            }),
            diagnostics,
        };

        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("crosshook {version}");
        println!();
        println!(
            "Profiles: {} ({})",
            profile_names.len(),
            profiles_dir.as_deref().unwrap_or("<unknown>")
        );
        for name in &profile_names {
            println!("  - {name}");
        }
        println!();
        println!("Steam roots: {}", steam_roots.len());
        for root in &steam_roots {
            println!("  {}", root.display());
        }
        println!();
        println!("Proton installs: {}", proton_installs.len());
        for install in &proton_installs {
            println!("  {} ({})", install.name, install.path.display());
        }
        if let Some(settings) = settings_data {
            println!();
            println!("Settings:");
            println!(
                "  auto_load_last_profile: {}",
                settings.auto_load_last_profile
            );
            println!("  last_used_profile:      {}", settings.last_used_profile);
            println!(
                "  community_taps:         {}",
                settings.community_taps.len()
            );
            println!(
                "  onboarding_completed:   {}",
                settings.onboarding_completed
            );
        }
    }

    Ok(())
}

fn profile_store(profile_dir: Option<PathBuf>) -> Result<ProfileStore, CliError> {
    match profile_dir {
        Some(path) => Ok(ProfileStore::with_base_path(path)),
        None => {
            let settings_store = SettingsStore::try_new()
                .map_err(|e| CliError::General(format!("settings store: {e}")))?;
            let settings = settings_store
                .load()
                .map_err(|e| CliError::General(format!("settings load: {e}")))?;
            ProfileStore::try_new_with_settings_data(&settings, &settings_store.base_path).map_err(
                |e| CliError::General(format!("failed to initialize profile store: {e}")),
            )
        }
    }
}

fn launch_request_from_profile(
    profile: &GameProfile,
    profile_name: &str,
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
        gamescope: profile.launch.gamescope.clone(),
        trainer_gamescope: if profile.launch.trainer_gamescope.is_default() {
            None
        } else {
            Some(profile.launch.trainer_gamescope.clone())
        },
        mangohud: profile.launch.mangohud.clone(),
    })
}

// TODO: dedup with core version when signatures align
// core: crosshook_core::launch::runtime_helpers::resolve_steam_client_install_path(&str) -> Option<String>
// (takes the configured steam_client_install_path, not the compatdata path)
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
    let mut command = launch::script_runner::build_helper_command(request, helper_script, log_path);
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

fn format_field_state(state: SteamAutoPopulateFieldState) -> &'static str {
    match state {
        SteamAutoPopulateFieldState::Found => "Found",
        SteamAutoPopulateFieldState::NotFound => "not detected",
        SteamAutoPopulateFieldState::Ambiguous => "Ambiguous — set manually",
    }
}
