import type { LaunchOptimizations } from './launch-optimizations';
import type { LaunchMethod, TrainerLoadingMode } from './profile';

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
  trainer_loading_mode: TrainerLoadingMode;
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

export type LaunchValidationSeverity = 'fatal' | 'warning' | 'info';

export interface LaunchValidationIssue {
  message: string;
  help: string;
  severity: LaunchValidationSeverity;
}

export type LaunchFeedback =
  | { kind: 'validation'; issue: LaunchValidationIssue }
  | { kind: 'runtime'; message: string };

export function isLaunchValidationIssue(value: unknown): value is LaunchValidationIssue {
  if (typeof value !== 'object' || value === null) {
    return false;
  }

  const candidate = value as Partial<LaunchValidationIssue>;

  return (
    typeof candidate.message === 'string' &&
    typeof candidate.help === 'string' &&
    (candidate.severity === 'fatal' ||
      candidate.severity === 'warning' ||
      candidate.severity === 'info')
  );
}

export interface LaunchResult {
  succeeded: boolean;
  message: string;
  helper_log_path: string;
}
