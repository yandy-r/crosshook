import type { Handler } from '../index';
import { getStore } from '../store';
import { emitMockEvent } from '../eventBus';
import type {
  CachedHealthSnapshot,
  EnrichedHealthSummary,
  EnrichedProfileHealthReport,
  HealthIssue,
  HealthStatus,
} from '../../../types/health';
import type { CachedOfflineReadinessSnapshot } from '../../../types/offline';
import type {
  VersionCheckResult,
  VersionCorrelationStatus,
  VersionSnapshotInfo,
} from '../../../types/version';

// ---------------------------------------------------------------------------
// Module-scope health state
// ---------------------------------------------------------------------------

// Keyed by profile name; populated by batch_validate_profiles and get_profile_health
const healthSnapshots = new Map<string, EnrichedProfileHealthReport>();

// Tracks which profile names have had their version changes acknowledged
const acknowledgedVersions = new Set<string>();

// ---------------------------------------------------------------------------
// Synthetic data helpers
// ---------------------------------------------------------------------------

function nowIso(): string {
  return new Date().toISOString();
}

function daysAgoIso(days: number): string {
  const d = new Date();
  d.setDate(d.getDate() - days);
  return d.toISOString();
}

function buildHealthReport(
  profileName: string,
  status: HealthStatus,
  issues: HealthIssue[],
): EnrichedProfileHealthReport {
  return {
    name: profileName,
    status,
    launch_method: 'steam',
    issues,
    checked_at: nowIso(),
    metadata: {
      profile_id: `mock-pid-${profileName.replace(/\s+/g, '-').toLowerCase()}`,
      last_success: daysAgoIso(2),
      failure_count_30d: 0,
      total_launches: 5,
      launcher_drift_state: null,
      is_community_import: false,
      is_favorite: false,
      version_status: acknowledgedVersions.has(profileName) ? 'matched' : 'game_updated',
      snapshot_build_id: '12345678',
      current_build_id: '12345678',
      trainer_version: null,
    },
    offline_readiness: null,
  };
}

function buildBatchSummary(profiles: EnrichedProfileHealthReport[]): EnrichedHealthSummary {
  const healthy_count = profiles.filter((p) => p.status === 'healthy').length;
  const stale_count = profiles.filter((p) => p.status === 'stale').length;
  const broken_count = profiles.filter((p) => p.status === 'broken').length;
  return {
    profiles,
    healthy_count,
    stale_count,
    broken_count,
    total_count: profiles.length,
    validated_at: nowIso(),
  };
}

function getSeededProfiles(): EnrichedProfileHealthReport[] {
  const store = getStore();
  const names = Array.from(store.profiles.keys());

  if (healthSnapshots.size > 0) {
    return names.map((name) => {
      const existing = healthSnapshots.get(name);
      if (existing) return existing;
      const fresh = buildHealthReport(name, 'healthy', []);
      healthSnapshots.set(name, fresh);
      return fresh;
    });
  }

  // First call — synthesize per-profile statuses with variety
  const statuses: HealthStatus[] = ['healthy', 'stale'];
  const result = names.map((name, i) => {
    const status = statuses[i % statuses.length] ?? 'healthy';
    const issues: HealthIssue[] =
      status === 'stale'
        ? [
            {
              field: 'trainer.path',
              path: '/mock/trainers/dev-game-beta.exe',
              message: 'Trainer binary not found at the configured path',
              remediation: 'Re-select the trainer executable in the profile editor',
              severity: 'warning',
            },
          ]
        : [];
    const report = buildHealthReport(name, status, issues);
    healthSnapshots.set(name, report);
    return report;
  });

  return result;
}

function buildVersionSnapshot(profileName: string): VersionSnapshotInfo {
  return {
    profile_id: `mock-pid-${profileName.replace(/\s+/g, '-').toLowerCase()}`,
    steam_app_id: '9999001',
    steam_build_id: '12345678',
    trainer_version: null,
    trainer_file_hash: null,
    human_game_ver: null,
    status: acknowledgedVersions.has(profileName) ? 'matched' : 'game_updated',
    checked_at: daysAgoIso(1),
  };
}

function buildVersionCheckResult(profileName: string): VersionCheckResult {
  const profileId = `mock-pid-${profileName.replace(/\s+/g, '-').toLowerCase()}`;
  const status: VersionCorrelationStatus = acknowledgedVersions.has(profileName)
    ? 'matched'
    : 'game_updated';
  return {
    profile_id: profileId,
    current_build_id: '12345678',
    snapshot: buildVersionSnapshot(profileName),
    status,
    update_in_progress: false,
  };
}

// ---------------------------------------------------------------------------
// Handler registration
// ---------------------------------------------------------------------------

export function registerHealth(map: Map<string, Handler>): void {
  // batch_validate_profiles — validates all profiles and emits the batch-complete event
  map.set('batch_validate_profiles', async () => {
    const profiles = getSeededProfiles();
    const summary = buildBatchSummary(profiles);
    // Emit event after a short delay so the hook's event listener fires after mount
    setTimeout(() => {
      emitMockEvent('profile-health-batch-complete', summary);
    }, 400);
    return summary;
  });

  // get_profile_health — single profile health check
  map.set('get_profile_health', async (args) => {
    const { name } = args as { name: string };
    const store = getStore();
    if (!store.profiles.has(name)) {
      throw new Error(`[dev-mock] profile not found: ${name}`);
    }
    const existing = healthSnapshots.get(name);
    if (existing) return existing;
    const report = buildHealthReport(name, 'healthy', []);
    healthSnapshots.set(name, report);
    return report;
  });

  // get_cached_health_snapshots — returns advisory snapshot list
  map.set('get_cached_health_snapshots', async (): Promise<CachedHealthSnapshot[]> => {
    const store = getStore();
    const names = Array.from(store.profiles.keys());
    return names.map((name) => {
      const snap = healthSnapshots.get(name);
      return {
        profile_id: `mock-pid-${name.replace(/\s+/g, '-').toLowerCase()}`,
        profile_name: name,
        status: (snap?.status ?? 'healthy') as CachedHealthSnapshot['status'],
        issue_count: snap?.issues.length ?? 0,
        checked_at: snap?.checked_at ?? daysAgoIso(1),
      };
    });
  });

  // get_cached_offline_readiness_snapshots — returns cached offline readiness rows
  map.set(
    'get_cached_offline_readiness_snapshots',
    async (): Promise<CachedOfflineReadinessSnapshot[]> => {
      const store = getStore();
      const names = Array.from(store.profiles.keys());
      return names.map((name) => ({
        profile_id: `mock-pid-${name.replace(/\s+/g, '-').toLowerCase()}`,
        profile_name: name,
        readiness_state: 'ready',
        readiness_score: 90,
        trainer_type: 'standalone',
        trainer_present: 1,
        trainer_hash_valid: 1,
        trainer_activated: 1,
        proton_available: 1,
        community_tap_cached: 0,
        network_required: 0,
        blocking_reasons: null,
        checked_at: daysAgoIso(1),
      }));
    },
  );

  // check_version_status — checks version correlation for a named profile
  map.set('check_version_status', async (args) => {
    const { name } = args as { name: string };
    const store = getStore();
    if (!store.profiles.has(name)) {
      throw new Error(`[dev-mock] profile not found: ${name}`);
    }
    const result = buildVersionCheckResult(name);
    // Emit version-scan-complete after a short delay
    setTimeout(() => {
      emitMockEvent('version-scan-complete', { scanned: 1, mismatches: 0 });
    }, 300);
    return result;
  });

  // get_version_snapshot — returns the latest version snapshot for a profile
  map.set('get_version_snapshot', async (args): Promise<VersionSnapshotInfo | null> => {
    const { name } = args as { name: string };
    const store = getStore();
    if (!store.profiles.has(name)) {
      return null;
    }
    return buildVersionSnapshot(name);
  });

  // set_trainer_version — records a manual trainer version hint
  map.set('set_trainer_version', async (args) => {
    const { name, version } = args as { name: string; version: string };
    const store = getStore();
    if (!store.profiles.has(name)) {
      throw new Error(
        `[dev-mock] profile '${name}' is not registered in the metadata store`,
      );
    }
    // In the mock we just note the version was set; no persistent state change needed
    void version;
    return null;
  });

  // acknowledge_version_change — marks the latest version change as acknowledged
  map.set('acknowledge_version_change', async (args) => {
    const { name } = args as { name: string };
    const store = getStore();
    if (!store.profiles.has(name)) {
      throw new Error(
        `[dev-mock] profile '${name}' is not registered in the metadata store`,
      );
    }
    acknowledgedVersions.add(name);
    // Update the in-memory snapshot so subsequent reads reflect acknowledged state
    const existing = healthSnapshots.get(name);
    if (existing?.metadata) {
      healthSnapshots.set(name, {
        ...existing,
        metadata: { ...existing.metadata, version_status: 'matched' },
      });
    }
    return null;
  });
}
