import type { LaunchOptimizations } from './launch-optimizations';
import type { LaunchMethod } from './profile';

export enum LaunchPhase {
  Idle = "Idle",
  GameLaunching = "GameLaunching",
  WaitingForTrainer = "WaitingForTrainer",
  TrainerLaunching = "TrainerLaunching",
  SessionActive = "SessionActive",
}

export interface LaunchRequest {
  method: Exclude<LaunchMethod, ''>;
  game_path: string;
  trainer_path: string;
  trainer_host_path: string;
  steam: {
    app_id: string;
    compatdata_path: string;
    proton_path: string;
    steam_client_install_path: string;
  };
  runtime: {
    prefix_path: string;
    proton_path: string;
    working_directory: string;
  };
  optimizations: LaunchOptimizations;
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
