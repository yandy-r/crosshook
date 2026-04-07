// These types mirror the Rust DTOs in crosshook-core/src/protonup/mod.rs

export type ProtonUpProvider = 'ge-proton' | 'proton-cachyos';

export interface ProtonUpAvailableVersion {
  provider: string;
  version: string;
  release_url?: string;
  download_url?: string;
  checksum_url?: string;
  checksum_kind?: string;
  asset_size?: number;
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
