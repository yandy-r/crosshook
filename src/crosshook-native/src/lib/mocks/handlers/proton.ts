import type {
  ApplyMigrationRequest,
  BatchMigrationRequest,
  BatchMigrationResult,
  MigrationApplyResult,
  MigrationScanResult,
} from '../../../types/migration';
import type { ProtonInstallOption } from '../../../types/proton';
import { getStore } from '../store';
import { emitProfilesChanged } from './profile-utils';
import type { Handler } from './types';

const MOCK_PROTON_INSTALLS: ProtonInstallOption[] = [
  {
    name: 'GE-Proton9-1',
    path: '/mock/compatibilitytools.d/GE-Proton9-1',
    is_official: false,
  },
  {
    name: 'GE-Proton9-2',
    path: '/mock/compatibilitytools.d/GE-Proton9-2',
    is_official: false,
  },
  {
    name: 'Proton - Experimental',
    path: '/mock/SteamLibrary/steamapps/common/Proton - Experimental',
    is_official: true,
  },
];

/**
 * Mirrors core `apply_single_migration`: a missing profile is `failed`, otherwise
 * the requested Proton path field is rewritten and the result is `applied`.
 */
function applyMockMigration(request: ApplyMigrationRequest): MigrationApplyResult {
  const store = getStore();
  const profile = store.profiles.get(request.profile_name);
  if (!profile) {
    return {
      profile_name: request.profile_name,
      field: request.field,
      old_path: '',
      new_path: request.new_path,
      outcome: 'failed',
      error: `[dev-mock] profile not found: ${request.profile_name}`,
    };
  }
  const updated = structuredClone(profile);
  let oldPath: string;
  if (request.field === 'steam_proton_path') {
    oldPath = updated.steam.proton_path;
    updated.steam = { ...updated.steam, proton_path: request.new_path };
  } else {
    oldPath = updated.runtime.proton_path;
    updated.runtime = { ...updated.runtime, proton_path: request.new_path };
  }
  store.profiles.set(request.profile_name, updated);
  return {
    profile_name: request.profile_name,
    field: request.field,
    old_path: oldPath || '/mock/old-proton-path',
    new_path: request.new_path,
    outcome: 'applied',
    error: null,
  };
}

export function registerProton(map: Map<string, Handler>): void {
  map.set('list_proton_installs', async (): Promise<ProtonInstallOption[]> => {
    return MOCK_PROTON_INSTALLS;
  });

  map.set('check_proton_migrations', async (): Promise<MigrationScanResult> => {
    return {
      suggestions: [],
      unmatched: [],
      profiles_scanned: 0,
      affected_count: 0,
      installed_proton_versions: MOCK_PROTON_INSTALLS.map((install) => ({
        name: install.name,
        path: install.path,
        is_official: install.is_official,
      })),
      diagnostics: [],
    };
  });

  map.set('apply_proton_migration', async (args): Promise<MigrationApplyResult> => {
    const { request } = args as { request: ApplyMigrationRequest };
    if (!request?.profile_name || !request?.field || !request?.new_path) {
      throw new Error('[dev-mock] apply_proton_migration: request must include profile_name, field, and new_path');
    }
    const result = applyMockMigration(request);
    if (result.outcome === 'applied') {
      emitProfilesChanged({ reason: { kind: 'migrated' }, names: [result.profile_name] });
    }
    return result;
  });

  map.set('apply_batch_migration', async (args): Promise<BatchMigrationResult> => {
    const { request } = args as { request: BatchMigrationRequest };
    if (!request?.migrations) {
      throw new Error('[dev-mock] apply_batch_migration: request.migrations is required');
    }
    // Pre-flight mirrors the backend: any unloadable profile aborts with zero writes.
    const store = getStore();
    for (const m of request.migrations) {
      if (!store.profiles.has(m.profile_name)) {
        throw new Error(
          `[dev-mock] apply_batch_migration: Pre-flight failed for '${m.profile_name}': profile not found`
        );
      }
    }
    const results = request.migrations.map(applyMockMigration);
    const appliedNames = results.filter((r) => r.outcome === 'applied').map((r) => r.profile_name);
    const failedCount = results.filter((r) => r.outcome === 'failed').length;
    if (appliedNames.length > 0) {
      emitProfilesChanged({ reason: { kind: 'migrated' }, names: appliedNames });
    }
    return {
      results,
      applied_count: appliedNames.length,
      failed_count: failedCount,
      skipped_count: results.length - appliedNames.length - failedCount,
    };
  });
}
