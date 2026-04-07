import { useCallback, useEffect, useRef, useState } from 'react';
import { callCommand } from '@/lib/ipc';

import type { LibraryCardData, ProfileSummary } from '../types/library';

export interface UseLibrarySummariesResult {
  summaries: LibraryCardData[];
  setSummaries: React.Dispatch<React.SetStateAction<LibraryCardData[]>>;
  loading: boolean;
  error: string | null;
}

/**
 * Fetches profile summaries from the backend and enriches them with favorite
 * state. Uses a ref for `favoriteProfiles` during fetch so re-fetches are not
 * triggered by favorite toggles, while a separate effect keeps the favorite
 * flag in sync when `favoriteProfiles` changes independently.
 */
export function useLibrarySummaries(
  profiles: string[],
  favoriteProfiles: string[],
): UseLibrarySummariesResult {
  const [summaries, setSummaries] = useState<LibraryCardData[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Ref keeps current favorites accessible without adding a dependency that
  // would re-trigger the fetch on every favorite toggle.
  const favoriteProfilesRef = useRef(favoriteProfiles);
  favoriteProfilesRef.current = favoriteProfiles;

  const fetchSummaries = useCallback(async () => {
    try {
      const result = await callCommand<ProfileSummary[]>('profile_list_summaries');
      const favoriteSet = new Set(favoriteProfilesRef.current);
      setSummaries(
        result.map((s) => ({
          name: s.name,
          gameName: s.gameName,
          steamAppId: s.steamAppId,
          customCoverArtPath: s.customCoverArtPath,
          customPortraitArtPath: s.customPortraitArtPath,
          isFavorite: favoriteSet.has(s.name),
        })),
      );
    } catch (err) {
      console.error('Failed to fetch profile summaries', err);
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  // Fetch on mount and whenever the profile list changes.
  useEffect(() => {
    void fetchSummaries();
  }, [profiles, fetchSummaries]);

  // Enrich with favorite state when favorites change (no network call).
  useEffect(() => {
    const favoriteSet = new Set(favoriteProfiles);
    setSummaries((prev) =>
      prev.map((s) => ({ ...s, isFavorite: favoriteSet.has(s.name) })),
    );
  }, [favoriteProfiles]);

  return { summaries, setSummaries, loading, error };
}
