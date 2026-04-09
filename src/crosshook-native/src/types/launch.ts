import type { DiagnosticReport } from './diagnostics';
import type { LaunchOptimizations } from './launch-optimizations';
import type { GamescopeConfig, LaunchMethod, MangoHudConfig, TrainerLoadingMode } from './profile';
import type { ResolvedLaunchMethod } from '../utils/launch';

/** Tone for the auto-save status indicator shared across Launch page tabs. */
export type LaunchAutoSaveStatusTone = 'idle' | 'saving' | 'success' | 'warning' | 'error';

/** Status of an auto-save operation (optimizations, gamescope, or mangohud). */
export interface LaunchAutoSaveStatus {
  tone: LaunchAutoSaveStatusTone;
  label: string;
  detail?: string;
}

export enum LaunchPhase {
  Idle = 'Idle',
  GameLaunching = 'GameLaunching',
  WaitingForTrainer = 'WaitingForTrainer',
  TrainerLaunching = 'TrainerLaunching',
  SessionActive = 'SessionActive',
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
  network_isolation: boolean;
  gamescope: GamescopeConfig;
  trainer_gamescope?: GamescopeConfig;
  mangohud: MangoHudConfig;
}

export type LaunchValidationSeverity = 'fatal' | 'warning' | 'info';

export interface LaunchValidationIssue {
  message: string;
  help: string;
  severity: LaunchValidationSeverity;
  /** Machine-readable kind: `trainer_hash_mismatch`, `trainer_hash_community_mismatch`, etc. */
  code?: string;
  trainer_hash_stored?: string;
  trainer_hash_current?: string;
  /** Community manifest expected digest when `code` is `trainer_hash_community_mismatch`. */
  trainer_sha256_community?: string;
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

  if (
    typeof candidate.message !== 'string' ||
    typeof candidate.help !== 'string' ||
    (candidate.severity !== 'fatal' && candidate.severity !== 'warning' && candidate.severity !== 'info')
  ) {
    return false;
  }
  if (candidate.code !== undefined && typeof candidate.code !== 'string') {
    return false;
  }
  if (candidate.trainer_hash_stored !== undefined && typeof candidate.trainer_hash_stored !== 'string') {
    return false;
  }
  if (candidate.trainer_hash_current !== undefined && typeof candidate.trainer_hash_current !== 'string') {
    return false;
  }
  if (
    candidate.trainer_sha256_community !== undefined &&
    typeof candidate.trainer_sha256_community !== 'string'
  ) {
    return false;
  }
  return true;
}

export interface LaunchResult {
  succeeded: boolean;
  message: string;
  helper_log_path: string;
  /** Advisory launch warnings (e.g. low offline readiness); launch still proceeds. */
  warnings?: LaunchValidationIssue[];
}

// --- Preview / Dry Run Types ---

export type EnvVarSource = 'proton_runtime' | 'launch_optimization' | 'host' | 'steam_proton' | 'profile_custom';

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
  umu_run_path: string | null;
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

export type { ResolvedLaunchMethod } from '../utils/launch';

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

/** Stable identifier union for pipeline nodes. */
export type PipelineNodeId =
  | 'game'
  | 'wine-prefix'
  | 'proton'
  | 'steam'
  | 'trainer'
  | 'optimizations'
  | 'launch';

/** Status of a single pipeline node. */
export type PipelineNodeStatus = 'configured' | 'not-configured' | 'error' | 'active' | 'complete';

/**
 * Live-launch presentation hint layered on top of `PipelineNodeStatus` (e.g. trainer handoff).
 * Does not widen `PipelineNodeStatus` so labels and CSS stay centralized.
 */
export type PipelineNodeTone = 'default' | 'waiting';

/** A node in the launch pipeline visualization. */
export interface PipelineNode {
  /** Stable identifier (e.g., 'game', 'wine-prefix', 'proton'). */
  id: PipelineNodeId;
  /** Display label (e.g., 'Game', 'Wine Prefix'). */
  label: string;
  /** Current status for visual rendering. */
  status: PipelineNodeStatus;
  /** Optional detail text (e.g., resolved path, error message). */
  detail?: string;
  /** Optional live-launch tone (e.g. waiting-for-trainer) while `status` remains `active`. */
  tone?: PipelineNodeTone;
}
