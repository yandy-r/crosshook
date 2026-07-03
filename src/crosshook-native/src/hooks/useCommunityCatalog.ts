import { useCallback, useEffect, useRef, useState } from 'react';
import { callCommand } from '@/lib/ipc';

import type { CatalogPage, CatalogQuery } from '../types/discovery';
import { useDebounce } from './useDebounce';

const SEARCH_DEBOUNCE_MS = 300;

export type FacetKey = 'gameTitle' | 'loadingMode' | 'compatibility' | 'tap';

const FACET_QUERY_KEYS: Record<FacetKey, 'gameTitles' | 'loadingModes' | 'compatibilityBands' | 'tapUrls'> = {
  gameTitle: 'gameTitles',
  loadingMode: 'loadingModes',
  compatibility: 'compatibilityBands',
  tap: 'tapUrls',
};

export interface UseCommunityCatalogReturn {
  data: CatalogPage | null;
  loading: boolean;
  error: string | null;
  query: CatalogQuery;
  /** Raw, un-debounced search input value. */
  searchText: string;
  /** Debounced (300 ms) into `query.query`. */
  setSearchText: (text: string) => void;
  /** Single-select v1: `undefined` clears the dimension. */
  setFacet: (key: FacetKey, value: string | undefined) => void;
  clearFilters: () => void;
  loadMore: () => Promise<void>;
  refresh: () => Promise<void>;
}

export function useCommunityCatalog(options?: { enabled?: boolean }): UseCommunityCatalogReturn {
  const enabled = options?.enabled ?? true;
  const [data, setData] = useState<CatalogPage | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState<CatalogQuery>({});
  const [searchText, setSearchTextState] = useState('');
  const requestIdRef = useRef(0);
  const hasDataRef = useRef(false);
  /** Query snapshot the current `data` was fetched with; guards loadMore
   * against appending onto a different query's page. */
  const dataQueryRef = useRef<CatalogQuery | null>(null);
  const searchDebounce = useDebounce(SEARCH_DEBOUNCE_MS);

  const fetchPage = useCallback(async (pageQuery: CatalogQuery, append: boolean): Promise<void> => {
    const id = ++requestIdRef.current;
    setLoading(true);
    setError(null);

    try {
      const page = await callCommand<CatalogPage>('discovery_catalog', { query: pageQuery });
      if (requestIdRef.current !== id) {
        return;
      }

      hasDataRef.current = true;
      if (!append) {
        dataQueryRef.current = pageQuery;
      }
      setData((previous) => (append && previous ? { ...page, entries: [...previous.entries, ...page.entries] } : page));
    } catch (err) {
      if (requestIdRef.current !== id) {
        return;
      }

      setError(err instanceof Error ? err.message : String(err));
      if (!hasDataRef.current) {
        setData(null);
      }
    } finally {
      if (requestIdRef.current === id) {
        setLoading(false);
      }
    }
  }, []);

  useEffect(() => {
    if (!enabled) {
      return;
    }

    void fetchPage(query, false);
  }, [enabled, query, fetchPage]);

  useEffect(
    () => () => {
      requestIdRef.current += 1;
    },
    []
  );

  const setSearchText = useCallback(
    (text: string) => {
      setSearchTextState(text);
      searchDebounce.schedule(() => {
        const trimmed = text.trim();
        setQuery((previous) => ({
          ...previous,
          query: trimmed.length > 0 ? trimmed : undefined,
          offset: undefined,
        }));
      });
    },
    [searchDebounce]
  );

  const setFacet = useCallback((key: FacetKey, value: string | undefined) => {
    setQuery((previous) => ({
      ...previous,
      [FACET_QUERY_KEYS[key]]: value === undefined ? [] : [value],
      offset: undefined,
    }));
  }, []);

  const clearFilters = useCallback(() => {
    setQuery((previous) => ({
      ...previous,
      gameTitles: [],
      loadingModes: [],
      compatibilityBands: [],
      tapUrls: [],
      offset: undefined,
    }));
  }, []);

  const loadMore = useCallback(async (): Promise<void> => {
    if (!enabled || !data || loading) {
      return;
    }
    if (dataQueryRef.current !== query) {
      return;
    }

    await fetchPage({ ...query, offset: data.entries.length }, true);
  }, [enabled, data, loading, query, fetchPage]);

  const refresh = useCallback(async (): Promise<void> => {
    if (!enabled) {
      return;
    }

    await fetchPage(query, false);
  }, [enabled, query, fetchPage]);

  return {
    data,
    loading,
    error,
    query,
    searchText,
    setSearchText,
    setFacet,
    clearFilters,
    loadMore,
    refresh,
  };
}
