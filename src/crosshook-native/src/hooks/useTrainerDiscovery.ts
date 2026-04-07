import { useCallback, useEffect, useRef, useState } from 'react';
import { callCommand } from '@/lib/ipc';

import type { TrainerSearchResponse } from '../types/discovery';

interface UseTrainerDiscoveryOptions {
  limit?: number;
  offset?: number;
}

export interface UseTrainerDiscoveryReturn {
  data: TrainerSearchResponse | null;
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
}

export function useTrainerDiscovery(
  query: string,
  options?: UseTrainerDiscoveryOptions,
): UseTrainerDiscoveryReturn {
  const [data, setData] = useState<TrainerSearchResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const requestIdRef = useRef(0);
  const debounceTimerRef = useRef<ReturnType<typeof setTimeout>>();

  const fetchResults = useCallback(
    async (searchQuery: string) => {
      const trimmed = searchQuery.trim();
      if (!trimmed) {
        setData(null);
        setLoading(false);
        setError(null);
        return;
      }

      const id = ++requestIdRef.current;
      setLoading(true);
      setError(null);

      try {
        const response = await callCommand<TrainerSearchResponse>('discovery_search_trainers', {
          query: {
            query: trimmed,
            limit: options?.limit,
            offset: options?.offset,
          },
        });

        if (requestIdRef.current !== id) {
          return;
        }

        setData(response);
      } catch (err) {
        if (requestIdRef.current !== id) {
          return;
        }

        setError(err instanceof Error ? err.message : String(err));
        setData(null);
      } finally {
        if (requestIdRef.current === id) {
          setLoading(false);
        }
      }
    },
    [options?.limit, options?.offset],
  );

  useEffect(() => {
    if (debounceTimerRef.current) {
      clearTimeout(debounceTimerRef.current);
    }

    const trimmed = query.trim();
    if (!trimmed) {
      requestIdRef.current += 1;
      setData(null);
      setLoading(false);
      setError(null);
      return;
    }

    debounceTimerRef.current = setTimeout(() => {
      void fetchResults(query);
    }, 300);

    return () => {
      if (debounceTimerRef.current) {
        clearTimeout(debounceTimerRef.current);
      }
    };
  }, [query, fetchResults]);

  const refresh = useCallback(async (): Promise<void> => {
    await fetchResults(query);
  }, [query, fetchResults]);

  return { data, loading, error, refresh };
}
