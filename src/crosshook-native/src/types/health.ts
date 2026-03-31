import type { OfflineReadinessBrief } from './offline';
import type { VersionCorrelationStatus } from './version';

export type HealthStatus = 'healthy' | 'stale' | 'broken';
export type HealthIssueSeverity = 'error' | 'warning' | 'info';

export interface HealthIssue {
  field: string;
  path: string;
  message: string;
  remediation: string;
  severity: HealthIssueSeverity;
}

export interface ProfileHealthReport {
  name: string;
  status: HealthStatus;
  launch_method: string;
  issues: HealthIssue[];
  checked_at: string;
}

export interface HealthCheckSummary {
  profiles: ProfileHealthReport[];
  healthy_count: number;
  stale_count: number;
  broken_count: number;
  total_count: number;
  validated_at: string;
}

export interface ProfileHealthMetadata {
  profile_id: string | null;
  last_success: string | null;
  failure_count_30d: number;
  total_launches: number;
  launcher_drift_state: string | null;
  is_community_import: boolean;
  is_favorite?: boolean;
  version_status?: VersionCorrelationStatus | null;
  snapshot_build_id?: string | null;
  current_build_id?: string | null;
  trainer_version?: string | null;
}

export interface EnrichedProfileHealthReport extends ProfileHealthReport {
  metadata: ProfileHealthMetadata | null;
  offline_readiness?: OfflineReadinessBrief | null;
}

export interface EnrichedHealthSummary {
  profiles: EnrichedProfileHealthReport[];
  healthy_count: number;
  stale_count: number;
  broken_count: number;
  total_count: number;
  validated_at: string;
}

// Phase D: advisory cache of last-computed health status per profile
export interface CachedHealthSnapshot {
  profile_id: string;
  profile_name: string;
  status: HealthStatus;
  issue_count: number;
  checked_at: string;
}
