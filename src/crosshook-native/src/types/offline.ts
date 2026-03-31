import type { HealthIssue } from './health';

export type OfflineCapability =
  | 'full'
  | 'full_with_runtime'
  | 'conditional_key'
  | 'conditional_session'
  | 'online_only'
  | 'unknown';

export interface TrainerTypeEntry {
  id: string;
  display_name: string;
  offline_capability: OfflineCapability;
  requires_network: boolean;
  detection_hints: string[];
  score_cap: number | null;
  info_modal: string | null;
}

export interface OfflineReadinessBrief {
  profile_name: string;
  score: number;
  readiness_state: string;
  trainer_type: string;
  blocking_reasons: string[];
  checked_at: string;
}

export interface OfflineReadinessReport {
  profile_name: string;
  score: number;
  readiness_state: string;
  trainer_type: string;
  checks: HealthIssue[];
  blocking_reasons: string[];
  checked_at: string;
}

export interface HashVerifyResult {
  hash: string;
  from_cache: boolean;
  file_size: number;
}

export interface CachedOfflineReadinessSnapshot {
  profile_id: string;
  profile_name: string;
  readiness_state: string;
  readiness_score: number;
  trainer_type: string;
  trainer_present: number;
  trainer_hash_valid: number;
  trainer_activated: number;
  proton_available: number;
  community_tap_cached: number;
  network_required: number;
  blocking_reasons: string | null;
  checked_at: string;
}

export interface OfflineReadinessScanCompletePayload {
  total_profiles: number;
}
