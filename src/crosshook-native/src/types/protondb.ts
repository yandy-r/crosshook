import type { GameProfile } from './profile';
import type { VersionCorrelationStatus } from './version';

export type ProtonDbTier = 'platinum' | 'gold' | 'silver' | 'bronze' | 'borked' | 'native' | 'unknown' | (string & {});

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

// ── Community-driven config suggestion types ────────────────────────

export type SuggestionStatus = 'new' | 'already_applied' | 'conflict' | 'dismissed';

export interface CatalogSuggestionItem {
  catalogEntryId: string;
  label: string;
  description: string;
  envPairs: [string, string][];
  status: SuggestionStatus;
  supportingReportCount: number;
}

export interface EnvVarSuggestionItem {
  key: string;
  value: string;
  status: SuggestionStatus;
  supportingReportCount: number;
}

export interface LaunchOptionSuggestionItem {
  rawText: string;
  supportingReportCount: number;
}

export interface ProtonDbSuggestionSet {
  catalogSuggestions: CatalogSuggestionItem[];
  envVarSuggestions: EnvVarSuggestionItem[];
  launchOptionSuggestions: LaunchOptionSuggestionItem[];
  tier: ProtonDbTier;
  totalReports: number;
  isStale: boolean;
}

export type AcceptSuggestionRequest =
  | { kind: 'catalog'; profileName: string; catalogEntryId: string }
  | { kind: 'env_var'; profileName: string; envKey: string; envValue: string };

export interface AcceptSuggestionResult {
  updatedProfile: GameProfile;
  appliedKeys: string[];
  toggledOptionIds: string[];
}
