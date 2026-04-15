import type { UmuCsvCoverage, UmuDatabaseRefreshStatus } from '../../../types/launch';
import type { Handler } from './types';

// Browser-dev-mode fixture. An empty/short-circuited app_id reports Unknown;
// otherwise we default to Missing so the amber warning UI is exercised by
// default. Explicit allow-list for app ids the mock considers "covered" —
// expand if you want to test the green path in dev.
const MOCK_FOUND_APP_IDS = new Set<string>([
  '546590', // Ghost of Tsushima — real umu-database hit
  '2050650', // Resident Evil 4 Remake — real umu-database hit
]);

export function registerUmuDatabase(map: Map<string, Handler>): void {
  map.set(
    'refresh_umu_database',
    async (): Promise<UmuDatabaseRefreshStatus> => ({
      refreshed: true,
      cached_at: new Date(Date.now() - 8 * 3600 * 1000).toISOString(),
      source_url: 'https://raw.githubusercontent.com/Open-Wine-Components/umu-database/main/umu-database.csv',
      reason: 'mocked — no network fetch',
    })
  );

  map.set('check_umu_coverage', async (args): Promise<UmuCsvCoverage> => {
    const { appId } = (args ?? {}) as { appId?: string };
    const trimmed = (appId ?? '').trim();
    if (trimmed === '') return 'unknown';
    return MOCK_FOUND_APP_IDS.has(trimmed) ? 'found' : 'missing';
  });
}
