import { useCallback, useEffect, useState } from 'react';

import { callCommand } from '@/lib/ipc';

export interface UseCollectionMembersResult {
  memberNames: string[];
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
}

/**
 * Returns the member profile names for a single collection. Refreshes when
 * `collectionId` changes; exposes a manual `refresh()` for parent components
 * that mutate membership and need to re-sync.
 */
export function useCollectionMembers(collectionId: string | null): UseCollectionMembersResult {
  const [memberNames, setMemberNames] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    if (collectionId === null) {
      setMemberNames([]);
      return;
    }
    setLoading(true);
    try {
      const result = await callCommand<string[]>('collection_list_profiles', { collectionId });
      setMemberNames(result);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setMemberNames([]);
    } finally {
      setLoading(false);
    }
  }, [collectionId]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { memberNames, loading, error, refresh };
}
