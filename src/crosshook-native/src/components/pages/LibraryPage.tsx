import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

import type { AppRoute } from '../layout/Sidebar';
import type { LibraryCardData, LibraryViewMode } from '../../types/library';
import { useLibraryProfiles } from '../../hooks/useLibraryProfiles';
import { useProfileContext } from '../../context/ProfileContext';
import { LibraryToolbar } from '../library/LibraryToolbar';
import { LibraryGrid } from '../library/LibraryGrid';
import { LibraryArt } from '../layout/PageBanner';
import { PanelRouteDecor } from '../layout/PanelRouteDecor';

interface ProfileSummary {
  name: string;
  gameName: string;
  steamAppId: string;
  customCoverArtPath?: string;
}

const VIEW_MODE_KEY = 'crosshook.library.viewMode';

function loadViewMode(): LibraryViewMode {
  const stored = localStorage.getItem(VIEW_MODE_KEY);
  return stored === 'list' ? 'list' : 'grid';
}

interface LibraryPageProps {
  onNavigate?: (route: AppRoute) => void;
}

export function LibraryPage({ onNavigate }: LibraryPageProps) {
  const { profiles, favoriteProfiles, selectProfile, toggleFavorite, refreshProfiles } =
    useProfileContext();

  const [summaries, setSummaries] = useState<LibraryCardData[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [viewMode, setViewMode] = useState<LibraryViewMode>(loadViewMode);
  const [launchingName, setLaunchingName] = useState<string | undefined>();

  // Fetch summaries from backend (no favorites dependency — avoids re-fetch on toggle)
  const fetchSummaries = useCallback(async () => {
    try {
      const result = await invoke<ProfileSummary[]>('profile_list_summaries');
      setSummaries(result.map((s) => ({
        name: s.name,
        gameName: s.gameName,
        steamAppId: s.steamAppId,
        customCoverArtPath: s.customCoverArtPath,
        isFavorite: false,
      })));
    } catch (err) {
      console.error('Failed to fetch profile summaries', err);
    }
  }, []);

  // On mount + when profiles list changes
  useEffect(() => {
    void fetchSummaries();
    void refreshProfiles();
  }, [fetchSummaries, refreshProfiles]);

  useEffect(() => {
    void fetchSummaries();
  }, [profiles, fetchSummaries]);

  // Enrich with favorite state separately (no network call)
  useEffect(() => {
    const favoriteSet = new Set(favoriteProfiles);
    setSummaries((prev) =>
      prev.map((s) => ({ ...s, isFavorite: favoriteSet.has(s.name) })),
    );
  }, [favoriteProfiles]);

  // Persist view mode
  const handleViewModeChange = useCallback((mode: LibraryViewMode) => {
    setViewMode(mode);
    localStorage.setItem(VIEW_MODE_KEY, mode);
  }, []);

  // Filter by search
  const filtered = useLibraryProfiles(summaries, searchQuery);

  // Launch handler: select profile then navigate to launch page
  const handleLaunch = useCallback(
    async (name: string) => {
      setLaunchingName(name);
      try {
        await selectProfile(name);
        onNavigate?.('launch');
      } finally {
        setLaunchingName(undefined);
      }
    },
    [selectProfile, onNavigate],
  );

  // Edit handler: select profile then navigate to profiles page
  const handleEdit = useCallback(
    async (name: string) => {
      await selectProfile(name);
      onNavigate?.('profiles');
    },
    [selectProfile, onNavigate],
  );

  // Favorite handler: optimistic update
  const handleToggleFavorite = useCallback(
    (name: string, current: boolean) => {
      // Optimistic: immediately update local state
      setSummaries((prev) =>
        prev.map((s) => (s.name === name ? { ...s, isFavorite: !current } : s)),
      );
      // Fire backend call (context handles persistence)
      void toggleFavorite(name, !current).catch(() => {
        // Revert on error
        setSummaries((prev) =>
          prev.map((s) => (s.name === name ? { ...s, isFavorite: current } : s)),
        );
      });
    },
    [toggleFavorite],
  );

  return (
    <div className="crosshook-page-scroll-shell crosshook-page-scroll-shell--fill crosshook-page-scroll-shell--library">
      <PanelRouteDecor illustration={<LibraryArt />} />
      <div className="crosshook-library-page">
        <LibraryToolbar
          searchQuery={searchQuery}
          onSearchChange={setSearchQuery}
          viewMode={viewMode}
          onViewModeChange={handleViewModeChange}
        />
        <LibraryGrid
          profiles={filtered}
          onLaunch={handleLaunch}
          onEdit={handleEdit}
          onToggleFavorite={handleToggleFavorite}
          launchingName={launchingName}
          onNavigate={onNavigate}
        />
      </div>
    </div>
  );
}

export default LibraryPage;
