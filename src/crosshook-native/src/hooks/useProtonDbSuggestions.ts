import { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

import type {
  AcceptSuggestionRequest,
  AcceptSuggestionResult,
  ProtonDbSuggestionSet,
} from '../types/protondb';

export interface UseProtonDbSuggestionsResult {
  suggestionSet: ProtonDbSuggestionSet | null;
  loading: boolean;
  error: string | null;
  acceptSuggestion: (request: AcceptSuggestionRequest) => Promise<AcceptSuggestionResult>;
  dismissSuggestion: (suggestionKey: string) => void;
  refresh: () => Promise<void>;
}

export function useProtonDbSuggestions(
  appId: string,
  profileName: string,
): UseProtonDbSuggestionsResult {
  const [suggestionSet, setSuggestionSet] = useState<ProtonDbSuggestionSet | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const requestIdRef = useRef(0);

  const fetchSuggestions = useCallback(
    async (forceRefresh = false) => {
      if (!appId || !profileName) {
        setSuggestionSet(null);
        setLoading(false);
        setError(null);
        return;
      }

      const id = ++requestIdRef.current;
      setLoading(true);
      setError(null);

      try {
        const result = await invoke<ProtonDbSuggestionSet>('protondb_get_suggestions', {
          appId,
          profileName,
          forceRefresh,
        });

        if (requestIdRef.current !== id) {
          return;
        }

        setSuggestionSet(result);
      } catch (err) {
        if (requestIdRef.current !== id) {
          return;
        }

        setError(err instanceof Error ? err.message : String(err));
        setSuggestionSet(null);
      } finally {
        if (requestIdRef.current === id) {
          setLoading(false);
        }
      }
    },
    [appId, profileName],
  );

  useEffect(() => {
    if (!appId || !profileName) {
      requestIdRef.current += 1;
      setSuggestionSet(null);
      setLoading(false);
      setError(null);
      return;
    }

    void fetchSuggestions(false);
  }, [appId, profileName, fetchSuggestions]);

  const acceptSuggestion = useCallback(
    async (request: AcceptSuggestionRequest): Promise<AcceptSuggestionResult> => {
      const result = await invoke<AcceptSuggestionResult>('protondb_accept_suggestion', {
        request,
      });
      void fetchSuggestions(false);
      return result;
    },
    [fetchSuggestions],
  );

  const dismissSuggestion = useCallback(
    (suggestionKey: string): void => {
      invoke('protondb_dismiss_suggestion', { profileName, appId, suggestionKey }).catch(() => {});

      setSuggestionSet((prev) => {
        if (!prev) return prev;
        return {
          ...prev,
          catalogSuggestions: prev.catalogSuggestions.filter(
            (s) => s.catalogEntryId !== suggestionKey,
          ),
          envVarSuggestions: prev.envVarSuggestions.filter((s) => s.key !== suggestionKey),
          launchOptionSuggestions: prev.launchOptionSuggestions.filter(
            (s) => s.rawText !== suggestionKey,
          ),
        };
      });
    },
    [profileName, appId],
  );

  const refresh = useCallback(async (): Promise<void> => {
    await fetchSuggestions(true);
  }, [fetchSuggestions]);

  return {
    suggestionSet,
    loading,
    error,
    acceptSuggestion,
    dismissSuggestion,
    refresh,
  };
}
