import type { VersionCorrelationStatus } from './version';

export type ProtonDbTier =
  | 'platinum'
  | 'gold'
  | 'silver'
  | 'bronze'
  | 'borked'
  | 'native'
  | 'unknown'
  | (string & {});

export type ProtonDbLookupState = 'idle' | 'loading' | 'ready' | 'stale' | 'unavailable';
export type ProtonDbAdvisoryKind = 'note' | 'launch_option';

export interface ProtonDbCacheState {
  cache_key?: string;
  fetched_at: string;
  expires_at: string | null;
  from_cache: boolean;
  is_stale: boolean;
  is_offline: boolean;
}

export interface ProtonDbAdvisoryNote {
  kind: ProtonDbAdvisoryKind;
  source_label?: string;
  text?: string;
}

export interface ProtonDbLaunchOptionSuggestion {
  kind: ProtonDbAdvisoryKind;
  source_label?: string;
  text?: string;
  supporting_report_count?: number;
}

export interface ProtonDbEnvVarSuggestion {
  key: string;
  value: string;
  source_label?: string;
  supporting_report_count?: number;
}

export interface ProtonDbRecommendationGroup {
  group_id?: string;
  title?: string;
  summary?: string;
  notes?: ProtonDbAdvisoryNote[];
  env_vars?: ProtonDbEnvVarSuggestion[];
  launch_options?: ProtonDbLaunchOptionSuggestion[];
}

export interface ProtonDbSnapshot {
  app_id?: string;
  tier: ProtonDbTier;
  best_reported_tier?: ProtonDbTier | null;
  trending_tier?: ProtonDbTier | null;
  score?: number | null;
  confidence?: string | null;
  total_reports?: number | null;
  recommendation_groups?: ProtonDbRecommendationGroup[];
  source_url?: string;
  fetched_at?: string;
}

export interface ProtonDbLookupResult {
  app_id: string;
  state: ProtonDbLookupState;
  cache: ProtonDbCacheState | null;
  snapshot: ProtonDbSnapshot | null;
}

export interface ProtonDbVersionContext {
  version_status: VersionCorrelationStatus | null;
}
