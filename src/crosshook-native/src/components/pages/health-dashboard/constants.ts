import type { VersionCorrelationStatus } from '../../../types/version';

export type IssueCategory =
  | 'missing_executable'
  | 'missing_trainer'
  | 'missing_dll'
  | 'missing_proton'
  | 'missing_compatdata'
  | 'missing_prefix'
  | 'inaccessible_path'
  | 'other';

export type SortField =
  | 'name'
  | 'status'
  | 'issues'
  | 'last_success'
  | 'launch_method'
  | 'failures'
  | 'favorite'
  | 'version_status'
  | 'offline_score';
export type SortDirection = 'asc' | 'desc';
export type StatusFilter = 'all' | 'healthy' | 'stale' | 'broken';

export interface IssueCategoryCount {
  category: IssueCategory;
  label: string;
  count: number;
}

export type CardTrend = 'up' | 'down' | null;

export const CATEGORY_LABELS: Record<IssueCategory, string> = {
  missing_executable: 'Missing executable',
  missing_trainer: 'Missing trainer',
  missing_dll: 'Missing DLL',
  missing_proton: 'Missing/invalid Proton path',
  missing_compatdata: 'Inaccessible compatdata',
  missing_prefix: 'Missing prefix path',
  inaccessible_path: 'Inaccessible path',
  other: 'Other',
};

export const STATUS_RANK: Record<string, number> = { broken: 2, stale: 1, healthy: 0 };

export const VERSION_STATUS_RANK: Partial<Record<VersionCorrelationStatus, number>> = {
  both_changed: 3,
  game_updated: 2,
  trainer_changed: 2,
  matched: 1,
  update_in_progress: 0,
  untracked: -1,
  unknown: -1,
};

export const DRIFT_STATE_MESSAGES: Record<string, string> = {
  missing: 'Exported launcher not found',
  moved: 'Launcher has moved',
  stale: 'Launcher may be outdated',
};
