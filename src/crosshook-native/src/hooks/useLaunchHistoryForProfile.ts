import { useEffect, useState } from 'react';
import { callCommand } from '@/lib/ipc';
import type { LaunchHistoryEntry } from '@/types/library';

export interface UseLaunchHistoryForProfileResult {
  rows: LaunchHistoryEntry[] | null;
  error: string | null;
}

export function useLaunchHistoryForProfile(
  profileName: string | undefined,
  limit: number
): UseLaunchHistoryForProfileResult {
  const [rows, setRows] = useState<LaunchHistoryEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const trimmed = profileName?.trim() ?? '';
    if (!trimmed) {
      setRows([]);
      setError(null);
      return;
    }

    let active = true;
    void (async () => {
      setRows(null);
      setError(null);
      try {
        const next = await callCommand<LaunchHistoryEntry[]>('list_launch_history_for_profile', {
          profileName: trimmed,
          limit,
        });
        if (active) {
          setRows(next);
        }
      } catch (e) {
        if (active) {
          setError(e instanceof Error ? e.message : String(e));
          setRows([]);
        }
      }
    })();

    return () => {
      active = false;
    };
  }, [profileName, limit]);

  return { rows, error };
}
