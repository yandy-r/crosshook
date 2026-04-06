import { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

import type { ExternalTrainerSearchResponse } from '../types/discovery';

export interface UseExternalTrainerSearchReturn {
  data: ExternalTrainerSearchResponse | null;
  loading: boolean;
  error: string | null;
  search: (forceRefresh?: boolean) => Promise<void>;
}

/**
 * Exposes manual external trainer lookup via `search(forceRefresh)`.
 */
export function useExternalTrainerSearch(
  gameName: string,
  options?: { steamAppId?: string },
): UseExternalTrainerSearchReturn {
  const [data, setData] = useState<ExternalTrainerSearchResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const requestIdRef = useRef(0);
  const isMountedRef = useRef(true);

  useEffect(() => () => {
    isMountedRef.current = false;
    requestIdRef.current += 1;
  }, []);

  const fetchResults = useCallback(
    async (forceRefresh = false) => {
      const trimmed = gameName.trim();
      if (!trimmed) {
        setData(null);
        setLoading(false);
        setError(null);
        return;
      }

      const id = ++requestIdRef.current;
      if (!isMountedRef.current) {
        return;
      }
      setLoading(true);
      setError(null);

      try {
        const result = await invoke<ExternalTrainerSearchResponse>(
          'discovery_search_external',
          {
            query: {
              gameName: trimmed,
              steamAppId: options?.steamAppId,
              forceRefresh,
            },
          },
        );

        if (!isMountedRef.current || requestIdRef.current !== id) {
          return;
        }

        setData(result);
      } catch (err) {
        if (!isMountedRef.current || requestIdRef.current !== id) {
          return;
        }

        setError(err instanceof Error ? err.message : String(err));
        setData(null);
      } finally {
        if (isMountedRef.current && requestIdRef.current === id) {
          setLoading(false);
        }
      }
    },
    [gameName, options?.steamAppId],
  );

  const search = useCallback(
    async (forceRefresh = false): Promise<void> => {
      await fetchResults(forceRefresh);
    },
    [fetchResults],
  );

  return { data, loading, error, search };
}
