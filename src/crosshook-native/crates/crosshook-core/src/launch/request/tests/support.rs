use std::fs;
use std::path::Path;

use crate::launch::request::{
    LaunchOptimizationsRequest, LaunchRequest, RuntimeLaunchConfig, SteamLaunchConfig,
    METHOD_NATIVE, METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH,
};
use crate::profile::TrainerLoadingMode;

pub(super) struct RequestFixture {
    pub(super) _temp_dir: tempfile::TempDir,
    pub(super) game_path: String,
    pub(super) trainer_path: String,
    pub(super) compatdata_path: String,
    pub(super) proton_path: String,
    pub(super) steam_client_install_path: String,
}

pub(super) fn write_executable_file(path: &Path) {
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

pub(super) fn steam_request() -> (tempfile::TempDir, LaunchRequest) {
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

pub(super) fn proton_request() -> (tempfile::TempDir, LaunchRequest) {
    let (temp_dir, mut request) = steam_request();
    request.method = METHOD_PROTON_RUN.to_string();
    request.game_path = request.game_path.replace("game.sh", "game.exe");
    write_executable_file(Path::new(&request.game_path));
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

pub(super) fn native_request() -> (tempfile::TempDir, LaunchRequest) {
    let (temp_dir, mut request) = steam_request();
    request.method = METHOD_NATIVE.to_string();
    request.trainer_path.clear();
    request.trainer_host_path.clear();
    request.steam = SteamLaunchConfig::default();
    (temp_dir, request)
}
