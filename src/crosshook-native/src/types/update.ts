export interface UpdateGameRequest {
  profile_name: string;
  updater_path: string;
  proton_path: string;
  prefix_path: string;
  steam_client_install_path: string;
}

export interface UpdateGameResult {
  succeeded: boolean;
  message: string;
  helper_log_path: string;
  /** Job id of the updater run. */
  run_id: string;
}

export type UpdateGameValidationError =
  | 'UpdaterPathRequired'
  | 'UpdaterPathNotAbsolute'
  | 'UpdaterPathMissing'
  | 'UpdaterPathNotFile'
  | 'UpdaterPathNotWindowsExecutable'
  | 'ProtonPathRequired'
  | 'ProtonPathNotAbsolute'
  | 'ProtonPathMissing'
  | 'ProtonPathNotExecutable'
  | 'PrefixPathRequired'
  | 'PrefixPathNotAbsolute'
  | 'PrefixPathMissing'
  | 'PrefixPathNotDirectory';

/** Keep in sync with `UpdateGameValidationError::message()` in crosshook-core `update/models.rs`. */
export const UPDATE_GAME_VALIDATION_MESSAGES: Record<UpdateGameValidationError, string> = {
  UpdaterPathRequired: 'The updater executable path is required.',
  UpdaterPathNotAbsolute: 'The updater executable path must be an absolute path.',
  UpdaterPathMissing: 'The updater executable path does not exist.',
  UpdaterPathNotFile: 'The updater executable path must be a file.',
  UpdaterPathNotWindowsExecutable: 'The updater executable path must point to a Windows .exe file.',
  ProtonPathRequired: 'A Proton path is required.',
  ProtonPathNotAbsolute: 'The Proton path must be an absolute path.',
  ProtonPathMissing: 'The Proton path does not exist.',
  ProtonPathNotExecutable: 'The Proton path does not point to an executable file.',
  PrefixPathRequired: 'A prefix path is required.',
  PrefixPathNotAbsolute: 'The prefix path must be an absolute path.',
  PrefixPathMissing: 'The prefix path does not exist.',
  PrefixPathNotDirectory: 'The prefix path must be a directory.',
};

export const UPDATE_GAME_VALIDATION_FIELD: Record<UpdateGameValidationError, keyof UpdateGameRequest | null> = {
  UpdaterPathRequired: 'updater_path',
  UpdaterPathNotAbsolute: 'updater_path',
  UpdaterPathMissing: 'updater_path',
  UpdaterPathNotFile: 'updater_path',
  UpdaterPathNotWindowsExecutable: 'updater_path',
  ProtonPathRequired: 'proton_path',
  ProtonPathNotAbsolute: 'proton_path',
  ProtonPathMissing: 'proton_path',
  ProtonPathNotExecutable: 'proton_path',
  PrefixPathRequired: 'prefix_path',
  PrefixPathNotAbsolute: 'prefix_path',
  PrefixPathMissing: 'prefix_path',
  PrefixPathNotDirectory: 'prefix_path',
};

export type UpdateGameStage = 'idle' | 'preparing' | 'running_updater' | 'complete' | 'failed';

export interface UpdateGameValidationState {
  fieldErrors: Partial<Record<keyof UpdateGameRequest, string>>;
  generalError: string | null;
}
