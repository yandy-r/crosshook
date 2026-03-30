export type VersionCorrelationStatus =
  | 'matched'
  | 'game_updated'
  | 'trainer_changed'
  | 'both_changed'
  | 'untracked'
  | 'unknown'
  | 'update_in_progress';

export interface VersionSnapshotInfo {
  profile_id: string;
  steam_app_id: string;
  steam_build_id: string | null;
  trainer_version: string | null;
  trainer_file_hash: string | null;
  human_game_ver: string | null;
  status: VersionCorrelationStatus;
  checked_at: string;
}

export interface VersionCheckResult {
  profile_id: string;
  current_build_id: string | null;
  snapshot: VersionSnapshotInfo | null;
  status: VersionCorrelationStatus;
  update_in_progress: boolean;
}

export interface VersionScanComplete {
  scanned: number;
  mismatches: number;
}
