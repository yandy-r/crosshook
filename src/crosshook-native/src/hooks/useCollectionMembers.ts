import { useCallback, useEffect, useRef, useState } from 'react';

import { callCommand } from '@/lib/ipc';

export interface UseCollectionMembersResult {
  memberNames: string[];
  /** Collection id these members were fetched for; null when none or before a fetch completes. */
  membersForCollectionId: string | null;
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
  const [membersForCollectionId, setMembersForCollectionId] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const requestSeqRef = useRef(0);

  const refresh = useCallback(async () => {
    if (collectionId === null) {
      requestSeqRef.current += 1;
      setMemberNames([]);
      setMembersForCollectionId(null);
      setError(null);
      setLoading(false);
      return;
    }
    const targetId = collectionId;
    const requestId = ++requestSeqRef.current;
    setLoading(true);
    setMemberNames([]);
    setMembersForCollectionId(null);
    setError(null);
    try {
      const result = await callCommand<string[]>('collection_list_profiles', { collectionId: targetId });
      if (requestId !== requestSeqRef.current) {
        return;
      }
      setMemberNames(result);
      setMembersForCollectionId(targetId);
    } catch (err) {
      if (requestId !== requestSeqRef.current) {
        return;
      }
      setError(err instanceof Error ? err.message : String(err));
      setMemberNames([]);
    } finally {
      if (requestId === requestSeqRef.current) {
        setLoading(false);
      }
    }
  }, [collectionId]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { memberNames, membersForCollectionId, loading, error, refresh };
}
