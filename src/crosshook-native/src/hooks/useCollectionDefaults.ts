import { useCallback, useEffect, useRef, useState } from 'react';

import { callCommand } from '@/lib/ipc';
import type { CollectionDefaults } from '@/types/profile';

export interface UseCollectionDefaultsResult {
  /** Latest fetched defaults; `null` when the collection has none or before a fetch completes. */
  defaults: CollectionDefaults | null;
  /** Collection id these defaults were fetched for; null when none or before a fetch completes. */
  defaultsForCollectionId: string | null;
  loading: boolean;
  error: string | null;
  /** Manually re-fetch defaults (e.g. after an external mutation). */
  reload: () => Promise<void>;
  /**
   * Persist a new defaults payload. Pass `null` to clear (writes NULL to the
   * column). Resolves after the backend write succeeds and the local state has
   * been re-synced via `reload()`.
   */
  saveDefaults: (next: CollectionDefaults | null) => Promise<void>;
}

/**
 * Fetches and writes per-collection launch defaults via the
 * `collection_get_defaults` / `collection_set_defaults` Tauri commands.
 *
 * Race-safety mirrors `useCollectionMembers`: a `requestSeqRef` discards stale
 * responses when `collectionId` changes mid-flight or when `reload()` overlaps.
 *
 * Hook returns `defaults: null` when:
 * - The collection has no defaults set in the metadata DB.
 * - `collectionId` is `null` (the modal is closed / no selection).
 * - The fetch failed (the error is exposed in `error`).
 */
export function useCollectionDefaults(
  collectionId: string | null
): UseCollectionDefaultsResult {
  const [defaults, setDefaults] = useState<CollectionDefaults | null>(null);
  const [defaultsForCollectionId, setDefaultsForCollectionId] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const requestSeqRef = useRef(0);

  const reload = useCallback(async () => {
    if (collectionId === null) {
      requestSeqRef.current += 1;
      setDefaults(null);
      setDefaultsForCollectionId(null);
      setError(null);
      setLoading(false);
      return;
    }
    const targetId = collectionId;
    const requestId = ++requestSeqRef.current;
    setLoading(true);
    setError(null);
    try {
      const result = await callCommand<CollectionDefaults | null>('collection_get_defaults', {
        collectionId: targetId,
      });
      if (requestId !== requestSeqRef.current) return;
      setDefaults(result ?? null);
      setDefaultsForCollectionId(targetId);
    } catch (err) {
      if (requestId !== requestSeqRef.current) return;
      setError(err instanceof Error ? err.message : String(err));
      setDefaults(null);
    } finally {
      if (requestId === requestSeqRef.current) {
        setLoading(false);
      }
    }
  }, [collectionId]);

  useEffect(() => {
    void reload();
  }, [reload]);

  const saveDefaults = useCallback(
    async (next: CollectionDefaults | null) => {
      if (collectionId === null) {
        throw new Error('cannot save collection defaults without a collection id');
      }
      await callCommand<null>('collection_set_defaults', {
        collectionId,
        defaults: next,
      });
      await reload();
    },
    [collectionId, reload]
  );

  return {
    defaults,
    defaultsForCollectionId,
    loading,
    error,
    reload,
    saveDefaults,
  };
}
