import { useEffect, useState } from 'react';
import type { OptimizationCatalogPayload } from '../utils/optimization-catalog';
import { fetchOptimizationCatalog } from '../utils/optimization-catalog';

export interface UseLaunchOptimizationCatalogResult {
  catalog: OptimizationCatalogPayload | null;
  loading: boolean;
  error: string | null;
}

/** React hook that fetches and caches the optimization catalog from the backend. */
export function useLaunchOptimizationCatalog(): UseLaunchOptimizationCatalogResult {
  const [catalog, setCatalog] = useState<OptimizationCatalogPayload | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void fetchOptimizationCatalog()
      .then((c) => {
        if (!cancelled) {
          setCatalog(c);
          setLoading(false);
        }
      })
      .catch((err) => {
        if (!cancelled) {
          setError(String(err));
          setLoading(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return { catalog, loading, error };
}
