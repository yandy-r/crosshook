import { useCallback, useEffect, useState } from 'react';

import type { AppRoute } from '../layout/Sidebar';
import type { LibraryViewMode } from '../../types/library';
import { useLibraryProfiles } from '../../hooks/useLibraryProfiles';
import { useLibrarySummaries } from '../../hooks/useLibrarySummaries';
import { useOfflineReadiness } from '../../hooks/useOfflineReadiness';
import { useProfileContext } from '../../context/ProfileContext';
import { useProfileHealthContext } from '../../context/ProfileHealthContext';
import { LibraryToolbar } from '../library/LibraryToolbar';
import { LibraryGrid } from '../library/LibraryGrid';
import { GameDetailsModal } from '../library/GameDetailsModal';
import { useGameDetailsModalState } from '../library/useGameDetailsModalState';

const VIEW_MODE_KEY = 'crosshook.library.viewMode';

function loadViewMode(): LibraryViewMode {
  const stored = localStorage.getItem(VIEW_MODE_KEY);
  return stored === 'list' ? 'list' : 'grid';
}

interface LibraryPageProps {
  onNavigate?: (route: AppRoute) => void;
}

export function LibraryPage({ onNavigate }: LibraryPageProps) {
  const { profiles, favoriteProfiles, selectedProfile, selectProfile, toggleFavorite, refreshProfiles } =
    useProfileContext();

  const { summaries, setSummaries } = useLibrarySummaries(profiles, favoriteProfiles);
  const { healthByName, loading: healthLoading } = useProfileHealthContext();
  const offlineReadiness = useOfflineReadiness();
  const gameDetailsModal = useGameDetailsModalState();
  const [searchQuery, setSearchQuery] = useState('');
  const [viewMode, setViewMode] = useState<LibraryViewMode>(loadViewMode);
  const [launchingName, setLaunchingName] = useState<string | undefined>();

  // Refresh profile list from context on mount
  useEffect(() => {
    void refreshProfiles();
  }, [refreshProfiles]);

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
  const handleOpenGameDetails = useCallback(
    async (name: string) => {
      const card = summaries.find((s) => s.name === name);
      if (!card) {
        return;
      }
      await selectProfile(name);
      gameDetailsModal.openForCard(card);
    },
    [gameDetailsModal, selectProfile, summaries],
  );

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
    [setSummaries, toggleFavorite],
  );

  return (
    <div className="crosshook-page-scroll-shell crosshook-page-scroll-shell--fill crosshook-page-scroll-shell--library">
      <div className="crosshook-route-stack crosshook-library-page">
        <div className="crosshook-route-stack__body--fill crosshook-library-page__body">
          <div className="crosshook-route-card-host">
            <div className="crosshook-route-card-scroll">
              <div className="crosshook-library-page__content">
                <div className="crosshook-library-page__toolbar-bar">
                  <LibraryToolbar
                    searchQuery={searchQuery}
                    onSearchChange={setSearchQuery}
                    viewMode={viewMode}
                    onViewModeChange={handleViewModeChange}
                  />
                </div>
                <LibraryGrid
                  profiles={filtered}
                  selectedName={selectedProfile}
                  onOpenDetails={handleOpenGameDetails}
                  onLaunch={handleLaunch}
                  onEdit={handleEdit}
                  onToggleFavorite={handleToggleFavorite}
                  launchingName={launchingName}
                  onNavigate={onNavigate}
                />
              </div>
            </div>
          </div>
        </div>
      </div>
      <GameDetailsModal
        open={gameDetailsModal.open}
        summary={gameDetailsModal.summary}
        onClose={gameDetailsModal.close}
        healthByName={healthByName}
        healthLoading={healthLoading}
        offlineReportFor={offlineReadiness.reportForProfile}
        offlineError={offlineReadiness.error}
        onLaunch={handleLaunch}
        onEdit={handleEdit}
        onToggleFavorite={handleToggleFavorite}
        launchingName={launchingName}
      />
    </div>
  );
}

export default LibraryPage;
