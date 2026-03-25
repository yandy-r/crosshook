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

/** Keep in sync with `InstallGameValidationError::message()` in crosshook-core `install/models.rs`. */
export const INSTALL_GAME_VALIDATION_MESSAGES: Record<InstallGameValidationError, string> = {
  ProfileNameRequired: 'An install profile name is required.',
  ProfileNameInvalid: 'The install profile name contains invalid characters.',
  InstallerPathRequired: 'An installer path is required.',
  InstallerPathMissing: 'The installer path does not exist.',
  InstallerPathNotFile: 'The installer path must be a file.',
  InstallerPathNotWindowsExecutable: 'The installer path must point to a Windows .exe file.',
  TrainerPathMissing: 'The trainer path does not exist.',
  TrainerPathNotFile: 'The trainer path must be a file.',
  ProtonPathRequired: 'A Proton path is required.',
  ProtonPathMissing: 'The Proton path does not exist.',
  ProtonPathNotExecutable: 'The Proton path must be executable.',
  PrefixPathRequired: 'A prefix path is required.',
  PrefixPathMissing: 'The prefix path does not exist.',
  PrefixPathNotDirectory: 'The prefix path must be a directory.',
  InstalledGameExecutablePathMissing: 'The final game executable path does not exist.',
  InstalledGameExecutablePathNotFile: 'The final game executable path must be a file.',
};

export const INSTALL_GAME_VALIDATION_FIELD: Record<InstallGameValidationError, keyof InstallGameRequest | null> = {
  ProfileNameRequired: 'profile_name',
  ProfileNameInvalid: 'profile_name',
  InstallerPathRequired: 'installer_path',
  InstallerPathMissing: 'installer_path',
  InstallerPathNotFile: 'installer_path',
  InstallerPathNotWindowsExecutable: 'installer_path',
  TrainerPathMissing: 'trainer_path',
  TrainerPathNotFile: 'trainer_path',
  ProtonPathRequired: 'proton_path',
  ProtonPathMissing: 'proton_path',
  ProtonPathNotExecutable: 'proton_path',
  PrefixPathRequired: 'prefix_path',
  PrefixPathMissing: 'prefix_path',
  PrefixPathNotDirectory: 'prefix_path',
  InstalledGameExecutablePathMissing: 'installed_game_executable_path',
  InstalledGameExecutablePathNotFile: 'installed_game_executable_path',
};

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
