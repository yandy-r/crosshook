export interface RunExecutableRequest {
  executable_path: string;
  proton_path: string;
  /** Optional. Empty string asks the backend to auto-resolve a throwaway prefix under `_run-adhoc/<slug>`. */
  prefix_path: string;
  working_directory: string;
  steam_client_install_path: string;
}

export interface RunExecutableResult {
  /**
   * `true` once the Proton wrapper has been *spawned* successfully. This
   * does NOT reflect the eventual exit status of the Windows executable —
   * that arrives asynchronously via the `run-executable-complete` event.
   */
  succeeded: boolean;
  message: string;
  helper_log_path: string;
  /** Echo of the prefix path the backend actually used (auto-resolved when `prefix_path` was empty). */
  resolved_prefix_path: string;
}

/**
 * Snake-case discriminants emitted by serde for
 * `RunExecutableValidationError` in `crosshook-core/src/run_executable/models.rs`.
 * The frontend never constructs these literally — it only matches on the
 * `variant` field of a `RunCommandError` payload returned by a Tauri
 * command, so changes here must move in lock-step with the Rust enum.
 */
export type RunExecutableValidationVariant =
  | 'executable_path_required'
  | 'executable_path_missing'
  | 'executable_path_not_file'
  | 'executable_path_not_windows_executable'
  | 'proton_path_required'
  | 'proton_path_missing'
  | 'proton_path_not_executable'
  | 'prefix_path_missing'
  | 'prefix_path_not_directory';

/**
 * Structured error envelope returned by every `run_executable` Tauri
 * command. Mirrors `RunCommandError` in
 * `src-tauri/src/commands/run_executable.rs`. The frontend matches on
 * `kind` instead of parsing message text.
 */
export type RunCommandError =
  | {
      kind: 'validation';
      variant: RunExecutableValidationVariant;
      field: keyof RunExecutableRequest;
      message: string;
    }
  | {
      kind: 'runtime';
      message: string;
    };

export type RunExecutableStage = 'idle' | 'preparing' | 'running' | 'complete' | 'failed';

export interface RunExecutableValidationState {
  fieldErrors: Partial<Record<keyof RunExecutableRequest, string>>;
  generalError: string | null;
}

/**
 * Type guard for the structured error envelope returned by Tauri commands.
 *
 * Tauri serializes a `Result<_, RunCommandError>` into a JSON object on the
 * promise rejection. We narrow defensively here so a transport-level failure
 * (which surfaces as a string) is still handled gracefully by callers.
 */
export function isRunCommandError(value: unknown): value is RunCommandError {
  if (typeof value !== 'object' || value === null) {
    return false;
  }
  const candidate = value as { kind?: unknown };
  return candidate.kind === 'validation' || candidate.kind === 'runtime';
}
