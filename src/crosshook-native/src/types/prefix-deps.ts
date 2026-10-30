/** State of a single prefix dependency package. */
export type DepState = 'unknown' | 'installed' | 'missing' | 'install_failed' | 'check_failed' | 'user_skipped';

/** Result of binary detection for winetricks/protontricks. */
export interface BinaryDetectionResult {
  found: boolean;
  binary_path: string | null;
  binary_name: string;
  tool_type: 'winetricks' | 'protontricks' | null;
  source: string;
}

/** Status of a single prefix dependency (from IPC). */
export interface PrefixDependencyStatus {
  package_name: string;
  state: DepState;
  checked_at: string | null;
  installed_at: string | null;
  last_error: string | null;
}

/** Persisted recovery state for a prefix Windows-version restoration. */
export type PrefixVersionRestoreState = 'pending' | 'failed';

/** Outcome of an automatic (startup or pre-install) prefix recovery. */
export type PrefixRecoveryOutcome = 'restored' | 'failed';

/** Last automatic recovery of a prefix in this session. */
export interface PrefixLastRecovery {
  outcome: PrefixRecoveryOutcome;
  /** Sanitized failure detail; null when restored. */
  message: string | null;
}

/** Current repair requirement for a prefix (from IPC). */
export interface PrefixVersionRepairStatus {
  required: boolean;
  state: PrefixVersionRestoreState | null;
  last_error: string | null;
  /**
   * Some job holds this prefix's lock: a dependency install, version repair or
   * startup recovery, but also an ad-hoc executable run, an update or an
   * install. On its own this does not block anything; see {@link isPrefixBusy}.
   */
  in_progress: boolean;
  last_recovery: PrefixLastRecovery | null;
}

/**
 * The prefix is busy when a job holds it while a repair row exists: the row
 * is then provisional (e.g. a dependency install's pending restore) and is
 * not a repair request until the job finishes.
 */
export function isPrefixBusy(status: PrefixVersionRepairStatus): boolean {
  return status.in_progress && status.required;
}

/**
 * Outcome of the restoration phase that follows dependency installation.
 * `pending` means the restore never completed (the installer failed to spawn or
 * the job ended without a result), so the prefix still needs a repair.
 */
export type PrefixVersionRestoreOutcomeState = 'not_required' | 'succeeded' | 'failed' | 'pending';

/** Payload emitted when dependency installation and compatibility restoration finish. */
export interface PrefixDepCompletePayload {
  profile_name: string;
  prefix_path: string;
  /** Safe overall success retained for compatibility with older consumers. */
  succeeded: boolean;
  /** Alias of install_exit_code retained for compatibility. */
  exit_code: number | null;
  install_succeeded: boolean;
  install_exit_code: number | null;
  restore_state: PrefixVersionRestoreOutcomeState;
  restore_error: string | null;
}
