export type PrefixCleanupTargetKind = 'orphan_prefix' | 'stale_staged_trainer';

export interface PrefixCleanupTarget {
  kind: PrefixCleanupTargetKind;
  resolved_prefix_path: string;
  target_path: string;
}

export interface StaleStagedTrainerEntry {
  resolved_prefix_path: string;
  target_path: string;
  entry_name: string;
  total_bytes: number;
  modified_at: string | null;
}

export interface PrefixStorageEntry {
  resolved_prefix_path: string;
  total_bytes: number;
  staged_trainers_bytes: number;
  is_orphan: boolean;
  referenced_by_profiles: string[];
  stale_staged_trainers: StaleStagedTrainerEntry[];
}

export interface PrefixStorageScanResult {
  scanned_at: string;
  prefixes: PrefixStorageEntry[];
  orphan_targets: PrefixCleanupTarget[];
  stale_staged_targets: PrefixCleanupTarget[];
  inventory_incomplete: boolean;
}

export interface PrefixCleanupSkipped {
  target: PrefixCleanupTarget;
  reason: string;
}

export interface PrefixCleanupResult {
  deleted: PrefixCleanupTarget[];
  skipped: PrefixCleanupSkipped[];
  reclaimed_bytes: number;
}

export interface PrefixStorageSnapshotRow {
  id: string;
  resolved_prefix_path: string;
  total_bytes: number;
  staged_trainers_bytes: number;
  is_orphan: boolean;
  referenced_profiles_json: string;
  stale_staged_count: number;
  scanned_at: string;
}

export interface PrefixStorageCleanupAuditRow {
  id: string;
  target_kind: string;
  resolved_prefix_path: string;
  target_path: string;
  result: string;
  reason: string | null;
  reclaimed_bytes: number;
  created_at: string;
}

export interface PrefixStorageHistoryResponse {
  available: boolean;
  snapshots: PrefixStorageSnapshotRow[];
  audit: PrefixStorageCleanupAuditRow[];
}
