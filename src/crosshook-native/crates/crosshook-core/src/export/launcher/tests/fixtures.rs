use super::super::SteamExternalLauncherExportRequest;
use crate::profile::{GamescopeConfig, TrainerLoadingMode};

pub(super) fn make_gamescope_request(
    gamescope: GamescopeConfig,
    loading_mode: TrainerLoadingMode,
) -> SteamExternalLauncherExportRequest {
    SteamExternalLauncherExportRequest {
        method: "proton_run".to_string(),
        trainer_path: "/opt/trainers/MyGame.exe".to_string(),
        trainer_loading_mode: loading_mode,
        prefix_path: "/tmp/compatdata/12345".to_string(),
        proton_path: "/opt/proton/proton".to_string(),
        target_home_path: "/home/user".to_string(),
        gamescope,
        ..Default::default()
    }
}
