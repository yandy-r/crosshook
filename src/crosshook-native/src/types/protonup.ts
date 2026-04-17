// These types mirror the Rust DTOs in crosshook-core/src/protonup/mod.rs

export type ProtonUpProvider = 'ge-proton' | 'proton-cachyos' | 'proton-em';

export interface ProtonUpAvailableVersion {
  provider: string;
  version: string;
  release_url?: string;
  download_url?: string;
  checksum_url?: string;
  checksum_kind?: string;
  asset_size?: number;
  /** ISO-8601 UTC release timestamp from upstream GitHub. */
  published_at?: string | null;
}

export interface ProtonUpCacheMeta {
  stale: boolean;
  offline: boolean;
  fetched_at?: string;
  expires_at?: string;
}

export interface ProtonUpCatalogResponse {
  versions: ProtonUpAvailableVersion[];
  cache: ProtonUpCacheMeta;
}

export interface ProtonUpInstallRequest {
  provider: string;
  version: string;
  target_root: string;
  force?: boolean;
}

export type ProtonUpInstallErrorKind =
  | 'dependency_missing'
  | 'permission_denied'
  | 'checksum_failed'
  | 'network_error'
  | 'invalid_path'
  | 'already_installed'
  | 'unknown';

export interface ProtonUpInstallResult {
  success: boolean;
  installed_path?: string;
  error_kind?: ProtonUpInstallErrorKind;
  error_message?: string;
}

export type ProtonUpMatchStatus = 'matched' | 'missing' | 'unknown';

export interface ProtonUpSuggestion {
  status: ProtonUpMatchStatus;
  community_version?: string;
  matched_install_name?: string;
  recommended_version?: string;
}

// Types for the native Proton download manager (Issue #274).
// Mirror the Rust DTOs from the protonup::manager / protonup::resolver modules.

export interface ProtonUpProviderDescriptor {
  id: string;
  display_name: string;
  supports_install: boolean;
  checksum_kind: 'sha512-sidecar' | 'sha256-manifest' | 'none';
}

export type InstallRootKind = 'native-steam' | 'flatpak-steam';

export interface InstallRootDescriptor {
  kind: InstallRootKind;
  path: string;
  writable: boolean;
  reason?: string | null;
}

export interface ProtonInstallHandle {
  op_id: string;
}

export type ProtonInstallPhase =
  | 'resolving'
  | 'downloading'
  | 'verifying'
  | 'extracting'
  | 'finalizing'
  | 'done'
  | 'failed'
  | 'cancelled';

export interface ProtonInstallProgress {
  op_id: string;
  phase: ProtonInstallPhase;
  bytes_done: number;
  bytes_total?: number | null;
  message?: string | null;
}

export interface ProtonUninstallResult {
  success: boolean;
  conflicting_app_ids: string[];
  error_message?: string | null;
}

export interface ProtonUninstallPlanResult {
  success: boolean;
  conflicting_app_ids: string[];
  error_message?: string | null;
}
