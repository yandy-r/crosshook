import type { EnrichedHealthSummary } from './health';
import type { MetadataStoreStatus } from './metadata';
import type { OfflineReadinessScanCompletePayload } from './offline';
import type { VersionScanComplete } from './version';

export type JobState<T> =
  | { state: 'pending' }
  | { state: 'running' }
  | { state: 'complete'; data: T }
  | { state: 'failed'; message: string }
  | { state: 'skipped'; reason: string };

export interface OnboardingStartup {
  show: boolean;
  has_profiles: boolean;
}

export type FlatpakMode = 'native' | 'isolated' | 'host_xdg_shared';

export interface FlatpakMigrationReport {
  imported_config: boolean;
  imported_subtrees: string[];
  skipped_subtrees: string[];
  error: string | null;
}

export interface FlatpakStartup {
  mode: FlatpakMode;
  migration: FlatpakMigrationReport | null;
}

export interface DegradedStores {
  metadata: MetadataStoreStatus;
  metadata_error: string | null;
  settings_unreadable: boolean;
  logging_unavailable: boolean;
}

/** Same payload as the `offline-readiness-scan-complete` event. */
export type OfflineReadinessScanSummary = OfflineReadinessScanCompletePayload;

/** Same payload as the `version-scan-complete` event. */
export type VersionScanSummary = VersionScanComplete;

export interface UmuRefreshSummary {
  refreshed: boolean;
  cached_at: string | null;
  reason: string;
}

export type BackgroundProtectionState = 'not_applicable' | 'pending' | 'available' | 'degraded' | 'unavailable';

export interface StartupJobs {
  health: JobState<EnrichedHealthSummary>;
  offline_readiness: JobState<OfflineReadinessScanSummary>;
  version_scan: JobState<VersionScanSummary>;
  umu_refresh: JobState<UmuRefreshSummary>;
  background_portal: JobState<BackgroundProtectionState>;
}

export interface StartupReport {
  auto_load_profile: string | null;
  onboarding: OnboardingStartup;
  flatpak: FlatpakStartup;
  degraded: DegradedStores;
  jobs: StartupJobs;
}

/** Job identifiers carried by the generic `startup-job-finished` event (`StartupJobKind`, snake_case). */
export type StartupJobName =
  | 'health_batch'
  | 'offline_readiness'
  | 'version_scan'
  | 'umu_refresh'
  | 'background_portal';

/** Terminal outcome of a startup job; `JobState['state']` minus the non-terminal `pending`/`running`. */
export type StartupJobTerminalState = 'complete' | 'failed' | 'skipped';

/**
 * Payload of the `startup-job-finished` event, emitted for every terminal job outcome (including
 * `failed` and `skipped`). Success data comes from the legacy event or a fresh report;
 * failed/skipped consumers can fall back to their batch command.
 */
export interface StartupJobFinished {
  job: StartupJobName;
  state: StartupJobTerminalState;
}
