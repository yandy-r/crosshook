use std::path::{Path, PathBuf};
use std::process::Stdio;

use clap::{Parser, Subcommand};
use crosshook_core::launch::{self, SteamLaunchRequest};
use crosshook_core::profile::{GameProfile, ProfileStore};
use tokio::fs::OpenOptions;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::time::{sleep, Duration};

const DEFAULT_SCRIPTS_DIR: &str = "../../../CrossHookEngine.App/runtime-helpers";
const HELPER_SCRIPT_NAME: &str = "steam-launch-helper.sh";

#[derive(Debug, Parser)]
#[command(name = "crosshook", version, about = "CrossHook native headless launcher")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Launch(LaunchCommand),
}

#[derive(Debug, Parser)]
struct LaunchCommand {
    #[arg(long, value_name = "NAME")]
    profile: String,
    #[arg(long, value_name = "PATH")]
    profile_dir: Option<PathBuf>,
    #[arg(long, value_name = "PATH")]
    scripts_dir: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Launch(command) => launch_profile(command).await?,
    }

    Ok(())
}

async fn launch_profile(command: LaunchCommand) -> Result<(), Box<dyn std::error::Error>> {
    let store = profile_store(command.profile_dir);
    let profile = store.load(&command.profile)?;
    let request = steam_launch_request_from_profile(&profile)?;
    launch::validate(&request)?;

    let scripts_dir = command
        .scripts_dir
        .unwrap_or_else(default_scripts_dir);
    let helper_script = scripts_dir.join(HELPER_SCRIPT_NAME);
    let log_path = launch_log_path(&command.profile);

    let mut child = spawn_helper(&request, &helper_script, &log_path).await?;
    stream_helper_log(&mut child, &log_path).await?;

    let status = child.wait().await?;
    if !status.success() {
        return Err(format!("helper exited with status {status}").into());
    }

    Ok(())
}

fn profile_store(profile_dir: Option<PathBuf>) -> ProfileStore {
    match profile_dir {
        Some(path) => ProfileStore::with_base_path(path),
        None => ProfileStore::new(),
    }
}

fn steam_launch_request_from_profile(profile: &GameProfile) -> Result<SteamLaunchRequest, Box<dyn std::error::Error>> {
    let steam_client_install_path = resolve_steam_client_install_path(&profile.steam.compatdata_path);
    if steam_client_install_path.as_os_str().is_empty() {
        return Err("could not determine Steam client install path".into());
    }

    Ok(SteamLaunchRequest {
        game_path: profile.game.executable_path.clone(),
        trainer_path: profile.trainer.path.clone(),
        trainer_host_path: profile.trainer.path.clone(),
        steam_app_id: profile.steam.app_id.clone(),
        steam_compat_data_path: profile.steam.compatdata_path.clone(),
        steam_proton_path: profile.steam.proton_path.clone(),
        steam_client_install_path: steam_client_install_path.to_string_lossy().into_owned(),
        launch_trainer_only: false,
        launch_game_only: true,
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

fn launch_log_path(profile_name: &str) -> PathBuf {
    let safe_name = profile_name
        .chars()
        .map(|character| if character.is_ascii_alphanumeric() { character } else { '-' })
        .collect::<String>();

    PathBuf::from("/tmp/crosshook-logs").join(format!("{safe_name}.log"))
}

async fn spawn_helper(
    request: &SteamLaunchRequest,
    helper_script: &Path,
    log_path: &Path,
) -> Result<tokio::process::Child, Box<dyn std::error::Error>> {
    let mut command = launch::script_runner::build_helper_command(request, helper_script, log_path);
    command.stdout(Stdio::null());
    command.stderr(Stdio::null());
    Ok(command.spawn()?)
}

async fn stream_helper_log(
    child: &mut tokio::process::Child,
    log_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut offset = 0u64;
    let mut stdout = tokio::io::stdout();

    loop {
        if let Some(status) = child.try_wait()? {
            let _ = drain_log(log_path, offset, &mut stdout).await?;
            stdout.flush().await?;
            if status.success() {
                return Ok(());
            }
            return Err(format!("helper exited with status {status}").into());
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
) -> Result<u64, Box<dyn std::error::Error>> {
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
