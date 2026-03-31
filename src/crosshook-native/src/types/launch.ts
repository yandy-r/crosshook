import type { DiagnosticReport } from './diagnostics';
import type { LaunchOptimizations } from './launch-optimizations';
import type { GamescopeConfig, LaunchMethod, TrainerLoadingMode } from './profile';

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
  profile_name?: string;
  custom_env_vars: Record<string, string>;
  gamescope: GamescopeConfig;
}

export type LaunchValidationSeverity = 'fatal' | 'warning' | 'info';

export interface LaunchValidationIssue {
  message: string;
  help: string;
  severity: LaunchValidationSeverity;
}

export type LaunchFeedback =
  | { kind: 'validation'; issue: LaunchValidationIssue }
  | { kind: 'diagnostic'; report: DiagnosticReport }
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

// --- Preview / Dry Run Types ---

export type EnvVarSource =
  | 'proton_runtime'
  | 'launch_optimization'
  | 'host'
  | 'steam_proton'
  | 'profile_custom';

export interface PreviewEnvVar {
  key: string;
  value: string;
  source: EnvVarSource;
}

export interface ProtonSetup {
  wine_prefix_path: string;
  compat_data_path: string;
  steam_client_install_path: string;
  proton_executable: string;
}

export interface PreviewTrainerInfo {
  path: string;
  host_path: string;
  loading_mode: TrainerLoadingMode;
  staged_path: string | null;
}

export interface PreviewValidation {
  issues: LaunchValidationIssue[];
}

export type ResolvedLaunchMethod = Exclude<LaunchMethod, ''>;

export interface LaunchPreview {
  resolved_method: ResolvedLaunchMethod;
  validation: PreviewValidation;
  environment: PreviewEnvVar[] | null;
  cleared_variables: string[];
  wrappers: string[] | null;
  effective_command: string | null;
  directives_error: string | null;
  steam_launch_options: string | null;
  proton_setup: ProtonSetup | null;
  working_directory: string;
  game_executable: string;
  game_executable_name: string;
  trainer: PreviewTrainerInfo | null;
  generated_at: string;
  display_text: string;
}
