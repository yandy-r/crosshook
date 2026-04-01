import { invoke } from '@tauri-apps/api/core';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import type {
  SteamAppDetails,
  SteamMetadataLookupResult,
  SteamMetadataLookupState,
} from '../types/game-metadata';

const idleLookup = (appId = ''): SteamMetadataLookupResult => ({
  app_id: appId,
  state: 'idle',
  app_details: null,
  from_cache: false,
  is_stale: false,
});

const METADATA_LOOKUP_DEBOUNCE_MS = 400;

function normalizeAppId(appId: string): string {
  const trimmed = appId.trim();
  return /^\d+$/.test(trimmed) ? trimmed : '';
}

function unavailableLookup(appId: string): SteamMetadataLookupResult {
  return {
    app_id: appId,
    state: 'unavailable',
    app_details: null,
    from_cache: false,
    is_stale: false,
  };
}

function normalizeAppDetails(details: SteamAppDetails | null): SteamAppDetails | null {
  if (details == null) {
    return null;
  }

  return {
    name: details.name ?? null,
    short_description: details.short_description ?? null,
    header_image: details.header_image ?? null,
    genres: details.genres ?? [],
  };
}

function normalizeLookupResult(result: SteamMetadataLookupResult): SteamMetadataLookupResult {
  return {
    app_id: result.app_id ?? '',
    state: result.state ?? 'unavailable',
    app_details: normalizeAppDetails(result.app_details ?? null),
    from_cache: result.from_cache ?? false,
    is_stale: result.is_stale ?? false,
  };
}

function loadingLookup(
  appId: string,
  previous: SteamMetadataLookupResult | null
): SteamMetadataLookupResult {
  const canReusePrevious = previous?.app_id === appId;
  return {
    app_id: appId,
    state: 'loading',
    app_details: canReusePrevious ? previous.app_details : null,
    from_cache: canReusePrevious ? previous.from_cache : false,
    is_stale: canReusePrevious ? previous.is_stale : false,
  };
}

export interface UseGameMetadataResult {
  appId: string;
  state: SteamMetadataLookupState;
  loading: boolean;
  result: SteamMetadataLookupResult;
  appDetails: SteamAppDetails | null;
  fromCache: boolean;
  isStale: boolean;
  isUnavailable: boolean;
  refresh: () => Promise<void>;
}

export function useGameMetadata(steamAppId: string | undefined): UseGameMetadataResult {
  const normalizedAppId = useMemo(
    () => normalizeAppId(steamAppId ?? ''),
    [steamAppId]
  );
  const [result, setResult] = useState<SteamMetadataLookupResult>(() =>
    idleLookup(normalizedAppId)
  );
  const [loading, setLoading] = useState(false);
  const requestIdRef = useRef(0);

  const runLookup = useCallback(
    async (forceRefresh: boolean) => {
      if (!normalizedAppId) {
        setLoading(false);
        setResult(idleLookup());
        return;
      }

      const requestId = ++requestIdRef.current;
      setLoading(true);
      setResult((current) => loadingLookup(normalizedAppId, current));

      try {
        const data = await invoke<SteamMetadataLookupResult>('fetch_game_metadata', {
          appId: normalizedAppId,
          forceRefresh,
        });

        if (requestId !== requestIdRef.current) {
          return;
        }

        setResult(normalizeLookupResult(data));
      } catch (err) {
        if (requestId !== requestIdRef.current) {
          return;
        }

        console.error('Steam metadata lookup failed', {
          requestId,
          normalizedAppId,
          error: err,
        });
        setResult(unavailableLookup(normalizedAppId));
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
      setResult(idleLookup());
      return;
    }

    const timer = window.setTimeout(() => {
      void runLookup(false);
    }, METADATA_LOOKUP_DEBOUNCE_MS);

    return () => {
      window.clearTimeout(timer);
    };
  }, [normalizedAppId, runLookup]);

  const refresh = useCallback(async () => {
    await runLookup(true);
  }, [runLookup]);

  return {
    appId: normalizedAppId,
    state: result.state,
    loading,
    result,
    appDetails: result.app_details,
    fromCache: result.from_cache,
    isStale: result.state === 'stale' || result.is_stale === true,
    isUnavailable: result.state === 'unavailable',
    refresh,
  };
}
