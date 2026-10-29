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
   * `true` once the run job has been registered and the Proton wrapper is
   * being started. This does NOT reflect the eventual exit status of the
   * Windows executable — that arrives asynchronously via the
   * `run-executable-complete` event.
   */
  succeeded: boolean;
  message: string;
  helper_log_path: string;
  /** Echo of the prefix path the backend actually used (auto-resolved when `prefix_path` was empty). */
  resolved_prefix_path: string;
  /** Job id of this run; every `run-executable-*` event for the run carries it. */
  run_id: string;
}

/** Payload of `run-executable-log`: one output line of a run. */
export interface RunExecutableLogPayload {
  run_id: string;
  line: string;
}

/** Payload of `run-executable-complete`. `code` is null when the process was killed by `signal`. */
export interface RunExecutableCompletePayload {
  run_id: string;
  code: number | null;
  signal: number | null;
}

function isNullableInteger(value: unknown): value is number | null {
  return value === null || Number.isInteger(value);
}

/** Narrows an untrusted `run-executable-log` event payload. */
export function isRunExecutableLogPayload(value: unknown): value is RunExecutableLogPayload {
  if (typeof value !== 'object' || value === null) {
    return false;
  }
  const candidate = value as Record<string, unknown>;
  return typeof candidate.run_id === 'string' && typeof candidate.line === 'string';
}

/** Narrows an untrusted `run-executable-complete` event payload. */
export function isRunExecutableCompletePayload(value: unknown): value is RunExecutableCompletePayload {
  if (typeof value !== 'object' || value === null) {
    return false;
  }
  const candidate = value as Record<string, unknown>;
  return (
    typeof candidate.run_id === 'string' && isNullableInteger(candidate.code) && isNullableInteger(candidate.signal)
  );
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
