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

/** Current repair requirement for a prefix (from IPC). */
export interface PrefixVersionRepairStatus {
  required: boolean;
  state: PrefixVersionRestoreState | null;
  last_error: string | null;
}

/** Outcome of the restoration phase that follows dependency installation. */
export type PrefixVersionRestoreOutcomeState = 'not_required' | 'succeeded' | 'failed';

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
