import { useEffect, useState } from 'react';
import type { CommandArgumentCatalogPayload } from '@/types/launch-command-arguments';
import { fetchCommandArgumentCatalog } from '@/utils/command-argument-catalog';

export interface UseCommandArgumentCatalogResult {
  catalog: CommandArgumentCatalogPayload | null;
  loading: boolean;
  error: string | null;
}

/** React hook that fetches and caches the command-argument catalog from the backend. */
export function useCommandArgumentCatalog(): UseCommandArgumentCatalogResult {
  const [catalog, setCatalog] = useState<CommandArgumentCatalogPayload | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void fetchCommandArgumentCatalog()
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
