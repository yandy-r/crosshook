import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { callCommand } from '@/lib/ipc';

import type {
  ProtonDbCacheState,
  ProtonDbLookupResult,
  ProtonDbLookupState,
  ProtonDbRecommendationGroup,
  ProtonDbSnapshot,
} from '../types/protondb';

const idleLookup = (appId = ''): ProtonDbLookupResult => ({
  app_id: appId,
  state: 'idle',
  cache: null,
  snapshot: null,
});

function normalizeAppId(appId: string): string {
  return appId.trim();
}

function unavailableLookup(appId: string): ProtonDbLookupResult {
  return {
    app_id: appId,
    state: 'unavailable',
    cache: null,
    snapshot: null,
  };
}

function normalizeRecommendationGroup(group: ProtonDbRecommendationGroup): ProtonDbRecommendationGroup {
  return {
    group_id: group.group_id ?? '',
    title: group.title ?? '',
    summary: group.summary ?? '',
    notes: group.notes ?? [],
    env_vars: group.env_vars ?? [],
    launch_options: group.launch_options ?? [],
  };
}

function normalizeSnapshot(snapshot: ProtonDbSnapshot | null): ProtonDbSnapshot | null {
  if (snapshot == null) {
    return null;
  }

  return {
    ...snapshot,
    recommendation_groups: (snapshot.recommendation_groups ?? []).map(normalizeRecommendationGroup),
  };
}

function normalizeLookupResult(result: ProtonDbLookupResult): ProtonDbLookupResult {
  return {
    app_id: result.app_id ?? '',
    state: result.state ?? 'unavailable',
    cache: result.cache ?? null,
    snapshot: normalizeSnapshot(result.snapshot ?? null),
  };
}

function loadingLookup(appId: string, previous: ProtonDbLookupResult | null): ProtonDbLookupResult {
  const canReusePrevious = previous?.app_id === appId;
  return {
    app_id: appId,
    state: 'loading',
    cache: canReusePrevious ? previous.cache : null,
    snapshot: canReusePrevious ? previous.snapshot : null,
  };
}

export interface UseProtonDbLookupResult {
  appId: string;
  state: ProtonDbLookupState;
  loading: boolean;
  lookup: ProtonDbLookupResult;
  snapshot: ProtonDbSnapshot | null;
  cache: ProtonDbCacheState | null;
  recommendationGroups: ProtonDbRecommendationGroup[];
  fromCache: boolean;
  isStale: boolean;
  isUnavailable: boolean;
  refresh: () => Promise<void>;
}

export function useProtonDbLookup(appId: string): UseProtonDbLookupResult {
  const normalizedAppId = useMemo(() => normalizeAppId(appId), [appId]);
  const [lookup, setLookup] = useState<ProtonDbLookupResult>(() => idleLookup(normalizedAppId));
  const [loading, setLoading] = useState(false);
  const requestIdRef = useRef(0);

  const runLookup = useCallback(
    async (forceRefresh: boolean) => {
      if (!normalizedAppId) {
        setLoading(false);
        setLookup(idleLookup());
        return;
      }

      const requestId = ++requestIdRef.current;
      setLoading(true);
      setLookup((current) => loadingLookup(normalizedAppId, current));

      try {
        const result = await callCommand<ProtonDbLookupResult>('protondb_lookup', {
          appId: normalizedAppId,
          forceRefresh,
        });

        if (requestId !== requestIdRef.current) {
          return;
        }

        setLookup(normalizeLookupResult(result));
      } catch (err) {
        if (requestId !== requestIdRef.current) {
          return;
        }

        console.error('ProtonDB lookup failed', {
          requestId,
          normalizedAppId,
          error: err,
        });
        setLookup(unavailableLookup(normalizedAppId));
      } finally {
        if (requestId === requestIdRef.current) {
          setLoading(false);
        }
      }
    },
    [normalizedAppId]
  );

  useEffect(() => {
    if (!normalizedAppId) {
      requestIdRef.current += 1;
      setLoading(false);
      setLookup(idleLookup());
      return;
    }

    void runLookup(false);
  }, [normalizedAppId, runLookup]);

  const refresh = useCallback(async () => {
    await runLookup(true);
  }, [runLookup]);

  const cache = lookup.cache;
  const snapshot = lookup.snapshot;

  return {
    appId: normalizedAppId,
    state: lookup.state,
    loading,
    lookup,
    snapshot,
    cache,
    recommendationGroups: snapshot?.recommendation_groups ?? [],
    fromCache: cache?.from_cache ?? false,
    isStale: lookup.state === 'stale' || cache?.is_stale === true,
    isUnavailable: lookup.state === 'unavailable',
    refresh,
  };
}
