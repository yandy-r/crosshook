import { useCallback, useId, useState } from 'react';
import { callCommand } from '@/lib/ipc';
import type { UmuDatabaseRefreshStatus } from '@/types/launch';

/**
 * Wraps `refresh_umu_database` IPC for Settings (and similar) UIs.
 */
export function useUmuDatabaseRefresh() {
  const refreshStatusId = useId();
  const clearStatusId = useId();
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [isClearing, setIsClearing] = useState(false);
  const [lastRefreshStatus, setLastRefreshStatus] = useState<UmuDatabaseRefreshStatus | null>(null);
  const [lastClearStatus, setLastClearStatus] = useState<string | null>(null);

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

  const clearGameIdLookupCache = useCallback(async () => {
    setIsClearing(true);
    try {
      const deleted = await callCommand<number>('clear_umu_gameid_lookup_cache');
      setLastClearStatus(`Cleared ${deleted} cached row${deleted === 1 ? '' : 's'}.`);
    } catch (err) {
      setLastClearStatus(String(err));
    } finally {
      setIsClearing(false);
    }
  }, []);

  return {
    isRefreshing,
    isClearing,
    lastRefreshStatus,
    lastClearStatus,
    refreshStatusId,
    clearStatusId,
    refresh,
    clearGameIdLookupCache,
  };
}
