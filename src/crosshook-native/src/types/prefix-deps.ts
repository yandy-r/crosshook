/** State of a single prefix dependency package. */
export type DepState =
  | 'unknown'
  | 'installed'
  | 'missing'
  | 'install_failed'
  | 'check_failed'
  | 'user_skipped';

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
