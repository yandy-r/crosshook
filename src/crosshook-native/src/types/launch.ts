export enum LaunchPhase {
  Idle = "Idle",
  GameLaunching = "GameLaunching",
  WaitingForTrainer = "WaitingForTrainer",
  TrainerLaunching = "TrainerLaunching",
  SessionActive = "SessionActive",
}

export interface SteamLaunchRequest {
  game_path: string;
  trainer_path: string;
  trainer_host_path: string;
  steam_app_id: string;
  steam_compat_data_path: string;
  steam_proton_path: string;
  steam_client_install_path: string;
  launch_trainer_only: boolean;
  launch_game_only: boolean;
}

export interface ValidationResult {
  is_valid: boolean;
  error_message: string;
}

export interface LaunchResult {
  succeeded: boolean;
  message: string;
  helper_log_path: string;
}
