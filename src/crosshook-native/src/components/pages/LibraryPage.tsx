import { useCallback, useEffect, useMemo, useState } from 'react';
import { useInspectorSelection } from '@/context/InspectorSelectionContext';
import { useCollections } from '@/hooks/useCollections';
import { useProfileContext } from '../../context/ProfileContext';
import { useProfileHealthContext } from '../../context/ProfileHealthContext';
import { useCollectionMembers } from '../../hooks/useCollectionMembers';
import { useLibraryProfiles } from '../../hooks/useLibraryProfiles';
import { useLibrarySummaries } from '../../hooks/useLibrarySummaries';
import { useOfflineReadiness } from '../../hooks/useOfflineReadiness';
import {
  type LibraryFilterKey,
  type LibrarySortKey,
  type LibraryViewMode,
  libraryCardDataEqual,
} from '../../types/library';
import { CollectionAssignMenu } from '../collections/CollectionAssignMenu';
import { CollectionEditModal } from '../collections/CollectionEditModal';
import { RouteBanner } from '../layout/RouteBanner';
import type { AppRoute } from '../layout/Sidebar';
import { GameDetailsModal } from '../library/GameDetailsModal';
import { LibraryGrid } from '../library/LibraryGrid';
import { LibraryList } from '../library/LibraryList';
import { LibraryToolbar } from '../library/LibraryToolbar';
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
  const { profiles, favoriteProfiles, selectProfile, toggleFavorite, refreshProfiles, activeCollectionId } =
    useProfileContext();
  const {
    memberNames: activeCollectionMemberNames,
    membersForCollectionId: activeCollectionMembersFetchedFor,
    loading: activeCollectionMembersLoading,
  } = useCollectionMembers(activeCollectionId);

  const { summaries, setSummaries } = useLibrarySummaries(profiles, favoriteProfiles, activeCollectionId);
  const { healthByName, loading: healthLoading } = useProfileHealthContext();
  const { setInspectorSelection, setLibraryInspectorHandlers } = useInspectorSelection();
  const offlineReadiness = useOfflineReadiness();
  const gameDetailsModal = useGameDetailsModalState();
  const [searchQuery, setSearchQuery] = useState('');
  const [viewMode, setViewMode] = useState<LibraryViewMode>(loadViewMode);
  const [sortBy, setSortBy] = useState<LibrarySortKey>('recent');
  const [filterKey, setFilterKey] = useState<LibraryFilterKey>('all');
  const [inspectorPickName, setInspectorPickName] = useState<string | null>(null);
  const [launchingName, setLaunchingName] = useState<string | undefined>();
  // `restoreFocusTo` is co-located with the open/close state so that all
  // fields update atomically in a single setState call. The element's
  // lifecycle matches the menu's (card stays mounted while the menu is open),
  // so no memory-leak concern applies. CollectionAssignMenu guards against
  // stale references with an `isConnected` check before calling `.focus()`.
  const [assignMenuState, setAssignMenuState] = useState<{
    open: boolean;
    profileName: string | null;
    anchorPosition: { x: number; y: number } | null;
    restoreFocusTo: HTMLElement | null;
  }>({ open: false, profileName: null, anchorPosition: null, restoreFocusTo: null });
  const [createCollectionFromMenuOpen, setCreateCollectionFromMenuOpen] = useState(false);
  const [createCollectionSessionError, setCreateCollectionSessionError] = useState<string | null>(null);
  const { createCollection } = useCollections();

  // Refresh profile list from context on mount
  useEffect(() => {
    void refreshProfiles();
  }, [refreshProfiles]);

  // Persist view mode
  const handleViewModeChange = useCallback((mode: LibraryViewMode) => {
    setViewMode(mode);
    localStorage.setItem(VIEW_MODE_KEY, mode);
  }, []);

  const searched = useLibraryProfiles(summaries, searchQuery);

  const displayedProfiles = useMemo(() => {
    let list = [...searched];
    switch (filterKey) {
      case 'favorites':
        list = list.filter((p) => p.isFavorite);
        break;
      case 'installed':
        list = list.filter((p) => Boolean(p.steamAppId && p.steamAppId !== '0'));
        break;
      default:
        break;
    }
    if (sortBy === 'name') {
      list.sort((a, b) => (a.gameName || a.name).localeCompare(b.gameName || b.name));
    }
    return list;
  }, [searched, filterKey, sortBy]);

  const handleCardSelect = useCallback((name: string) => {
    setInspectorPickName(name);
  }, []);

  // Launch handler: select profile then navigate to launch page.
  //
  // When an `activeCollectionId` is set globally (e.g. from the sidebar or a
  // recently-opened CollectionViewModal) AND the launched profile is actually
  // a member of that collection, thread the collection context so Rust's
  // `profile_load` applies the collection's launch defaults via
  // `effective_profile_with` (Phase 3 merge layer).
  //
  // If members haven't finished loading yet, or the profile isn't a member,
  // fall back to the raw load — matching the fail-open philosophy of the Rust
  // side. This prevents a stale collection context from silently leaking its
  // defaults into unrelated profile launches from the library grid.
  const handleLaunch = useCallback(
    async (name: string) => {
      setLaunchingName(name);
      try {
        const membersReady =
          activeCollectionId !== null &&
          !activeCollectionMembersLoading &&
          activeCollectionMembersFetchedFor === activeCollectionId;
        const profileIsInActiveCollection = membersReady && activeCollectionMemberNames.includes(name);
        const collectionIdForLoad = profileIsInActiveCollection ? (activeCollectionId ?? undefined) : undefined;
        await selectProfile(name, { collectionId: collectionIdForLoad });
        onNavigate?.('launch');
      } finally {
        setLaunchingName(undefined);
      }
    },
    [
      selectProfile,
      onNavigate,
      activeCollectionId,
      activeCollectionMemberNames,
      activeCollectionMembersFetchedFor,
      activeCollectionMembersLoading,
    ]
  );

  // Edit handler: select profile then navigate to profiles page
  const handleEdit = useCallback(
    async (name: string) => {
      await selectProfile(name);
      onNavigate?.('profiles');
    },
    [selectProfile, onNavigate]
  );

  // Favorite handler: optimistic update
  const handleOpenGameDetails = useCallback(
    async (name: string) => {
      const card = summaries.find((s) => s.name === name);
      if (!card) {
        return;
      }
      gameDetailsModal.openForCard(card);
      await selectProfile(name);
    },
    [gameDetailsModal, selectProfile, summaries]
  );

  const handleCardContextMenu = useCallback(
    (position: { x: number; y: number }, profileName: string, restoreFocusTo: HTMLElement) => {
      setAssignMenuState({
        open: true,
        profileName,
        anchorPosition: position,
        restoreFocusTo,
      });
    },
    []
  );

  const closeAssignMenu = useCallback(() => {
    setAssignMenuState({
      open: false,
      profileName: null,
      anchorPosition: null,
      restoreFocusTo: null,
    });
  }, []);

  const handleCreateFromAssignMenu = useCallback(() => {
    setCreateCollectionSessionError(null);
    setCreateCollectionFromMenuOpen(true);
  }, []);

  const handleSubmitCreateFromMenu = useCallback(
    async (name: string, description: string | null): Promise<boolean> => {
      setCreateCollectionSessionError(null);
      const result = await createCollection(name, description);
      if (!result.ok) {
        setCreateCollectionSessionError(result.error);
        return false;
      }
      if (result.descriptionFailed) {
        setCreateCollectionSessionError(
          `Collection created, but description could not be saved: ${result.descriptionFailed}`
        );
        return false;
      }
      return true;
    },
    [createCollection]
  );

  const handleToggleFavorite = useCallback(
    (name: string, current: boolean) => {
      // Optimistic: immediately update local state
      setSummaries((prev) => prev.map((s) => (s.name === name ? { ...s, isFavorite: !current } : s)));
      // Fire backend call (context handles persistence)
      void toggleFavorite(name, !current).catch(() => {
        // Revert on error
        setSummaries((prev) => prev.map((s) => (s.name === name ? { ...s, isFavorite: current } : s)));
      });
    },
    [setSummaries, toggleFavorite]
  );

  useEffect(() => {
    if (inspectorPickName == null) {
      setInspectorSelection(undefined);
      return;
    }
    const next = summaries.find((s) => s.name === inspectorPickName);
    setInspectorSelection((prev) => {
      if (prev === next) {
        return prev;
      }
      if (prev && next && libraryCardDataEqual(prev, next)) {
        return prev;
      }
      return next;
    });
  }, [inspectorPickName, summaries, setInspectorSelection]);

  useEffect(() => {
    return () => {
      setInspectorSelection(undefined);
    };
  }, [setInspectorSelection]);

  useEffect(() => {
    setLibraryInspectorHandlers({
      onLaunch: handleLaunch,
      onEditProfile: handleEdit,
      onToggleFavorite: handleToggleFavorite,
    });
    return () => setLibraryInspectorHandlers(undefined);
  }, [handleLaunch, handleEdit, handleToggleFavorite, setLibraryInspectorHandlers]);

  const handleOpenCommandPalette = useCallback(() => {
    console.debug('Command palette (Phase 6)');
  }, []);

  const activeGameDetailsSummary =
    gameDetailsModal.summary == null
      ? null
      : (summaries.find((summary) => summary.name === gameDetailsModal.summary?.name) ?? gameDetailsModal.summary);

  return (
    <div className="crosshook-page-scroll-shell crosshook-page-scroll-shell--fill crosshook-page-scroll-shell--library">
      <div className="crosshook-route-stack crosshook-library-page">
        <div className="crosshook-route-stack__body--fill crosshook-library-page__body">
          <RouteBanner route="library" />
          <div className="crosshook-route-card-host">
            <div className="crosshook-route-card-scroll">
              <div className="crosshook-card crosshook-library-page__content">
                <div className="crosshook-library-page__toolbar-bar">
                  <LibraryToolbar
                    searchQuery={searchQuery}
                    onSearchChange={setSearchQuery}
                    viewMode={viewMode}
                    onViewModeChange={handleViewModeChange}
                    sortBy={sortBy}
                    onSortChange={setSortBy}
                    filter={filterKey}
                    onFilterChange={setFilterKey}
                    onOpenCommandPalette={handleOpenCommandPalette}
                  />
                </div>
                {viewMode === 'grid' ? (
                  <LibraryGrid
                    profiles={displayedProfiles}
                    selectedName={inspectorPickName ?? undefined}
                    onSelect={handleCardSelect}
                    onOpenDetails={handleOpenGameDetails}
                    onLaunch={handleLaunch}
                    onEdit={handleEdit}
                    onToggleFavorite={handleToggleFavorite}
                    launchingName={launchingName}
                    onNavigate={onNavigate}
                    onContextMenu={handleCardContextMenu}
                  />
                ) : (
                  <LibraryList
                    profiles={displayedProfiles}
                    selectedName={inspectorPickName ?? undefined}
                    onSelect={handleCardSelect}
                    onOpenDetails={handleOpenGameDetails}
                    onLaunch={handleLaunch}
                    onEdit={handleEdit}
                    onToggleFavorite={handleToggleFavorite}
                    launchingName={launchingName}
                    onNavigate={onNavigate}
                    onContextMenu={handleCardContextMenu}
                  />
                )}
              </div>
            </div>
          </div>
        </div>
      </div>
      <GameDetailsModal
        open={gameDetailsModal.open}
        summary={activeGameDetailsSummary}
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
      <CollectionAssignMenu
        open={assignMenuState.open}
        profileName={assignMenuState.profileName}
        anchorPosition={assignMenuState.anchorPosition}
        restoreFocusTo={assignMenuState.restoreFocusTo}
        onClose={closeAssignMenu}
        onCreateNew={handleCreateFromAssignMenu}
      />
      <CollectionEditModal
        open={createCollectionFromMenuOpen}
        mode="create"
        onClose={() => {
          setCreateCollectionSessionError(null);
          setCreateCollectionFromMenuOpen(false);
        }}
        onSubmitCreate={handleSubmitCreateFromMenu}
        onSubmitEdit={async () => false}
        externalError={createCollectionSessionError}
      />
    </div>
  );
}

export default LibraryPage;
