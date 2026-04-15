import { useCallback, useState } from 'react';
import { callCommand } from '@/lib/ipc';
import type { UmuDatabaseRefreshStatus } from '@/types/launch';

/**
 * Wraps `refresh_umu_database` IPC for Settings (and similar) UIs.
 */
export function useUmuDatabaseRefresh() {
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [lastRefreshStatus, setLastRefreshStatus] = useState<UmuDatabaseRefreshStatus | null>(null);

  const refresh = useCallback(async () => {
    setIsRefreshing(true);
    try {
      const status = await callCommand<UmuDatabaseRefreshStatus>('refresh_umu_database');
      setLastRefreshStatus(status);
    } catch (err) {
      setLastRefreshStatus({
        refreshed: false,
        cached_at: null,
        source_url: '',
        reason: String(err),
      });
    } finally {
      setIsRefreshing(false);
    }
  }, []);

  return { isRefreshing, lastRefreshStatus, refresh };
}
