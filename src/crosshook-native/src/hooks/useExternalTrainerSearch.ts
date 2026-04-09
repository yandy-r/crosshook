import { useCallback, useEffect, useRef, useState } from 'react';
import { callCommand } from '@/lib/ipc';

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
  options?: { steamAppId?: string }
): UseExternalTrainerSearchReturn {
  const [data, setData] = useState<ExternalTrainerSearchResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const requestIdRef = useRef(0);
  const isMountedRef = useRef(true);

  useEffect(() => {
    // React StrictMode may run mount->cleanup->mount in dev; reset mounted state on setup.
    isMountedRef.current = true;
    return () => {
      isMountedRef.current = false;
      requestIdRef.current += 1;
    };
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
        const result = await callCommand<ExternalTrainerSearchResponse>('discovery_search_external', {
          query: {
            gameName: trimmed,
            steamAppId: options?.steamAppId,
            forceRefresh,
          },
        });

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
    [gameName, options?.steamAppId]
  );

  // Auto-fire with debounce when gameName changes.
  const debounceTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const clearDebounceTimer = useCallback(() => {
    if (debounceTimerRef.current) {
      clearTimeout(debounceTimerRef.current);
      debounceTimerRef.current = null;
    }
  }, []);

  useEffect(() => {
    clearDebounceTimer();

    const trimmed = gameName.trim();
    if (!trimmed) {
      requestIdRef.current += 1;
      setData(null);
      setLoading(false);
      setError(null);
      return;
    }

    // Longer debounce than local search (600ms) since this hits the network.
    debounceTimerRef.current = setTimeout(() => {
      debounceTimerRef.current = null;
      void fetchResults(false);
    }, 600);

    return () => {
      clearDebounceTimer();
    };
  }, [gameName, fetchResults, clearDebounceTimer]);

  const search = useCallback(
    async (forceRefresh = false): Promise<void> => {
      clearDebounceTimer();
      await fetchResults(forceRefresh);
    },
    [fetchResults, clearDebounceTimer]
  );

  return { data, loading, error, search };
}
