import type {
  ApplyMigrationRequest,
  BatchMigrationRequest,
  BatchMigrationResult,
  MigrationApplyResult,
  MigrationScanResult,
} from '../../../types/migration';
import type { ProtonInstallOption } from '../../../types/proton';
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
    return {
      profile_name: request.profile_name,
      field: request.field,
      old_path: '/mock/old-proton-path',
      new_path: request.new_path,
      outcome: 'applied',
      error: null,
    };
  });

  map.set('apply_batch_migration', async (args): Promise<BatchMigrationResult> => {
    const { request } = args as { request: BatchMigrationRequest };
    if (!request?.migrations) {
      throw new Error('[dev-mock] apply_batch_migration: request.migrations is required');
    }
    const results: MigrationApplyResult[] = request.migrations.map((m) => ({
      profile_name: m.profile_name,
      field: m.field,
      old_path: '/mock/old-proton-path',
      new_path: m.new_path,
      outcome: 'applied',
      error: null,
    }));
    return {
      results,
      applied_count: results.length,
      failed_count: 0,
      skipped_count: 0,
    };
  });
}
