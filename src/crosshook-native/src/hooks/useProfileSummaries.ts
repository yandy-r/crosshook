import { useCallback, useEffect, useState } from 'react';
import { callCommand } from '@/lib/ipc';

import type { ProfileSummary } from '../types/library';

export interface UseProfileSummariesResult {
  summaries: ProfileSummary[];
  loading: boolean;
  error: string | null;
}

export function useProfileSummaries(profiles: string[]): UseProfileSummariesResult {
  const [summaries, setSummaries] = useState<ProfileSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchSummaries = useCallback(async () => {
    setLoading(true);
    try {
      const rows = await callCommand<ProfileSummary[]>('profile_list_summaries');
      setSummaries(rows);
      setError(null);
    } catch (err) {
      console.error('Failed to fetch profile summaries', err);
      setSummaries([]);
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void fetchSummaries();
  }, [profiles, fetchSummaries]);

  return { summaries, loading, error };
}
