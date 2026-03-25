import type { GameProfile } from './profile';

export type ProfileReviewSource = 'install-complete' | 'manual-verify';

export interface InstallGameRequest {
  profile_name: string;
  display_name: string;
  installer_path: string;
  trainer_path: string;
  proton_path: string;
  prefix_path: string;
  installed_game_executable_path: string;
}

export interface InstallGameResult {
  succeeded: boolean;
  message: string;
  helper_log_path: string;
  profile_name: string;
  needs_executable_confirmation: boolean;
  discovered_game_executable_candidates: string[];
  profile: GameProfile;
}

export interface InstallProfileReviewPayload {
  source: ProfileReviewSource;
  profileName: string;
  generatedProfile: GameProfile;
  candidateOptions: InstallGameExecutableCandidate[];
  helperLogPath: string;
  message: string;
}

export interface InstallGameExecutableCandidate {
  path: string;
  is_recommended: boolean;
  index: number;
}

export type InstallGameValidationError =
  | 'ProfileNameRequired'
  | 'ProfileNameInvalid'
  | 'InstallerPathRequired'
  | 'InstallerPathMissing'
  | 'InstallerPathNotFile'
  | 'InstallerPathNotWindowsExecutable'
  | 'TrainerPathMissing'
  | 'TrainerPathNotFile'
  | 'ProtonPathRequired'
  | 'ProtonPathMissing'
  | 'ProtonPathNotExecutable'
  | 'PrefixPathRequired'
  | 'PrefixPathMissing'
  | 'PrefixPathNotDirectory'
  | 'InstalledGameExecutablePathMissing'
  | 'InstalledGameExecutablePathNotFile';

export type InstallGameStage =
  | 'idle'
  | 'preparing'
  | 'running_installer'
  | 'review_required'
  | 'ready_to_save'
  | 'failed';

export type InstallGamePrefixPathState = 'idle' | 'loading' | 'ready' | 'failed';

export interface InstallGameValidationState {
  fieldErrors: Partial<Record<keyof InstallGameRequest, string>>;
  generalError: string | null;
}
