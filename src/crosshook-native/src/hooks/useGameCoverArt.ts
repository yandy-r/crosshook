import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

function normalizeAppId(appId: string): string {
  return appId.trim();
}

export interface UseGameCoverArtResult {
  coverArtUrl: string | null;
  loading: boolean;
}

export function useGameCoverArt(
  steamAppId: string | undefined,
  customCoverArtPath?: string,
): UseGameCoverArtResult {
  const normalizedAppId = useMemo(
    () => normalizeAppId(steamAppId ?? ''),
    [steamAppId]
  );
  const [coverArtUrl, setCoverArtUrl] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const requestIdRef = useRef(0);

  const customUrl = useMemo(
    () => (customCoverArtPath?.trim() ? convertFileSrc(customCoverArtPath.trim()) : null),
    [customCoverArtPath]
  );

  const fetchCoverArt = useCallback(async () => {
    if (!normalizedAppId) {
      setCoverArtUrl(null);
      setLoading(false);
      return;
    }

    const requestId = ++requestIdRef.current;
    setLoading(true);
    setCoverArtUrl(null);

    try {
      const path = await invoke<string | null>('fetch_game_cover_art', {
        appId: normalizedAppId,
        imageType: 'cover',
      });

      if (requestId !== requestIdRef.current) {
        return;
      }

      setCoverArtUrl(path != null ? convertFileSrc(path) : null);
    } catch (err) {
      if (requestId !== requestIdRef.current) {
        return;
      }

      console.error('Game cover art fetch failed', {
        requestId,
        normalizedAppId,
        error: err,
      });
      setCoverArtUrl(null);
    } finally {
      if (requestId === requestIdRef.current) {
        setLoading(false);
      }
    }
  }, [normalizedAppId]);

  useEffect(() => {
    if (customUrl) {
      requestIdRef.current += 1;
      setCoverArtUrl(null);
      setLoading(false);
      return;
    }

    if (!normalizedAppId) {
      requestIdRef.current += 1;
      setCoverArtUrl(null);
      setLoading(false);
      return;
    }

    setCoverArtUrl(null);
    void fetchCoverArt();
  }, [normalizedAppId, fetchCoverArt, customUrl]);

  return { coverArtUrl: customUrl ?? coverArtUrl, loading: customUrl ? false : loading };
}
