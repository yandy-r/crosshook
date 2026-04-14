import { type MouseEvent, useCallback, useEffect, useId, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { gameDetailsEditThenNavigate, gameDetailsLaunchThenNavigate } from '@/components/library/game-details-actions';
import { LibraryCard } from '@/components/library/LibraryCard';
import { BROWSER_DEV_EXPORT_PRESET_PATH } from '@/constants/browserDevPresetPaths';
import { useProfileContext } from '@/context/ProfileContext';
import { useCollectionMembers } from '@/hooks/useCollectionMembers';
import { useCollections } from '@/hooks/useCollections';
import { useFocusTrap } from '@/hooks/useFocusTrap';
import { useLibraryProfiles } from '@/hooks/useLibraryProfiles';
import { useLibrarySummaries } from '@/hooks/useLibrarySummaries';
import { isBrowserDevUi } from '@/lib/runtime';
import type { CollectionRow } from '@/types/collections';
import type { LibraryCardData } from '@/types/library';
import { BrowserDevPresetExplainerModal } from './BrowserDevPresetExplainerModal';
import { CollectionLaunchDefaultsEditor } from './CollectionLaunchDefaultsEditor';
import './CollectionViewModal.css';

export interface CollectionViewModalProps {
  open: boolean;
  collectionId: string | null;
  onClose: () => void;
  onLaunch: (name: string) => void | Promise<void>;
  onEdit: (name: string) => void | Promise<void>;
  onRequestEditMetadata: (id: string) => void;
  launchingName?: string;
  /** When the active collection filter should clear (e.g. after delete). */
  onCollectionDeleted?: (collectionId: string) => void;
  /**
   * Phase 3: navigate to the Profiles page from inside the launch-defaults editor
   * link-out. The host preserves `activeCollectionId` so the Profiles page opens
   * inside the collection filter; the modal is expected to close as part of the
   * navigation.
   */
  onOpenInProfilesPage: () => void;
}

export function CollectionViewModal({
  open,
  collectionId,
  onClose,
  onLaunch,
  onEdit,
  onRequestEditMetadata,
  launchingName,
  onCollectionDeleted,
  onOpenInProfilesPage,
}: CollectionViewModalProps) {
  const titleId = useId();
  const descriptionId = useId();
  const surfaceRef = useRef<HTMLDivElement | null>(null);
  const headingRef = useRef<HTMLHeadingElement | null>(null);
  const portalHostRef = useRef<HTMLElement | null>(null);
  const [isMounted, setIsMounted] = useState(false);

  const { collections, deleteCollection, exportCollectionPreset } = useCollections();
  const { memberNames, loading: membersLoading } = useCollectionMembers(open ? collectionId : null);
  const { profiles, favoriteProfiles, selectedProfile } = useProfileContext();
  const { summaries } = useLibrarySummaries(profiles, favoriteProfiles);

  const [searchQuery, setSearchQuery] = useState('');
  const [deleteConfirm, setDeleteConfirm] = useState(false);
  const [exportError, setExportError] = useState<string | null>(null);
  const [exportExplainerOpen, setExportExplainerOpen] = useState(false);

  useEffect(() => {
    setSearchQuery('');
  }, []);

  useEffect(() => {
    if (!open) {
      setDeleteConfirm(false);
      setExportError(null);
      setExportExplainerOpen(false);
    }
  }, [open]);

  const handleExportPreset = useCallback(async () => {
    if (collectionId === null) {
      return;
    }
    setExportError(null);
    if (isBrowserDevUi()) {
      setExportExplainerOpen(true);
      return;
    }
    const result = await exportCollectionPreset(collectionId);
    if (result === 'cancelled') {
      return;
    }
    if (!result.ok) {
      setExportError(result.error);
    }
  }, [collectionId, exportCollectionPreset]);

  const handleExportExplainerContinue = useCallback(async () => {
    if (collectionId === null) {
      return;
    }
    setExportExplainerOpen(false);
    setExportError(null);
    const result = await exportCollectionPreset(collectionId, {
      outputPathOverride: BROWSER_DEV_EXPORT_PRESET_PATH,
    });
    if (result === 'cancelled') {
      return;
    }
    if (!result.ok) {
      setExportError(result.error);
    }
  }, [collectionId, exportCollectionPreset]);

  const collection = useMemo<CollectionRow | null>(
    () => collections.find((c) => c.collection_id === collectionId) ?? null,
    [collections, collectionId]
  );
  const collectionPresent = collection !== null;

  const memberSet = useMemo(() => new Set(memberNames), [memberNames]);
  const memberSummaries = useMemo<LibraryCardData[]>(
    () => summaries.filter((s) => memberSet.has(s.name)),
    [summaries, memberSet]
  );
  const filtered = useLibraryProfiles(memberSummaries, searchQuery);

  const handleLaunchClick = useCallback(
    (name: string) => {
      gameDetailsLaunchThenNavigate(name, onLaunch, onClose);
    },
    [onLaunch, onClose]
  );

  const handleEditClick = useCallback(
    (name: string) => {
      gameDetailsEditThenNavigate(name, onEdit, onClose);
    },
    [onEdit, onClose]
  );

  const handleDeleteCollection = useCallback(async () => {
    if (collectionId === null) {
      return;
    }
    const ok = await deleteCollection(collectionId);
    if (ok) {
      onCollectionDeleted?.(collectionId);
      onClose();
    }
  }, [collectionId, deleteCollection, onClose, onCollectionDeleted]);

  useEffect(() => {
    if (!open || typeof document === 'undefined') {
      return;
    }
    if (portalHostRef.current) {
      setIsMounted(true);
      return;
    }
    const host = document.createElement('div');
    host.className = 'crosshook-modal-portal';
    portalHostRef.current = host;
    document.body.appendChild(host);
    setIsMounted(true);
  }, [open]);

  useEffect(() => {
    return () => {
      if (portalHostRef.current) {
        portalHostRef.current.remove();
        portalHostRef.current = null;
        setIsMounted(false);
      }
    };
  }, []);

  const { handleKeyDown } = useFocusTrap({
    open: open && collectionPresent,
    panelRef: surfaceRef,
    onClose,
    initialFocusRef: headingRef,
  });

  function handleBackdropMouseDown(event: MouseEvent<HTMLDivElement>) {
    if (event.target !== event.currentTarget) {
      return;
    }
    onClose();
  }

  if (!open || !collection || !isMounted || !portalHostRef.current) {
    return null;
  }

  const mainModal = createPortal(
    <div className="crosshook-modal" role="presentation">
      <div className="crosshook-modal__backdrop" aria-hidden="true" onMouseDown={handleBackdropMouseDown} />
      <div
        ref={surfaceRef}
        className="crosshook-modal__surface crosshook-panel crosshook-focus-scope crosshook-collection-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={descriptionId}
        data-crosshook-focus-root="modal"
        onKeyDown={handleKeyDown}
      >
        <header className="crosshook-modal__header">
          <div className="crosshook-modal__heading-block">
            <h2 ref={headingRef} id={titleId} className="crosshook-modal__title" tabIndex={-1}>
              {collection.name}
            </h2>
            <p id={descriptionId} className="crosshook-modal__description">
              {collection.profile_count} profile{collection.profile_count === 1 ? '' : 's'}
              {collection.description ? ` · ${collection.description}` : ''}
            </p>
          </div>
          <div className="crosshook-modal__header-actions">
            <button
              type="button"
              className="crosshook-button crosshook-button--ghost"
              onClick={() => onRequestEditMetadata(collection.collection_id)}
            >
              Edit
            </button>
            <button
              type="button"
              className="crosshook-button crosshook-button--ghost crosshook-modal__close"
              data-crosshook-modal-close
              onClick={onClose}
            >
              Close
            </button>
          </div>
        </header>

        <div className="crosshook-modal__body crosshook-collection-modal__body">
          <CollectionLaunchDefaultsEditor
            collectionId={collection.collection_id}
            onOpenInProfilesPage={onOpenInProfilesPage}
          />

          <div className="crosshook-collection-modal__search">
            <label className="crosshook-label" htmlFor={`${titleId}-search`}>
              Search this collection
            </label>
            <input
              id={`${titleId}-search`}
              type="text"
              className="crosshook-input"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Type to filter…"
              aria-controls={`${titleId}-results`}
            />
          </div>

          {membersLoading ? (
            <p className="crosshook-collection-modal__status">Loading members…</p>
          ) : filtered.length === 0 ? (
            <div className="crosshook-collection-modal__empty">
              {memberSummaries.length === 0
                ? 'No profiles in this collection yet. Right-click a library card to add one.'
                : 'No profiles match your search.'}
            </div>
          ) : (
            <ul id={`${titleId}-results`} className="crosshook-collection-modal__grid crosshook-list-reset">
              {filtered.map((card) => (
                <LibraryCard
                  key={card.name}
                  profile={card}
                  isSelected={selectedProfile === card.name}
                  onOpenDetails={() => {
                    handleLaunchClick(card.name);
                  }}
                  onLaunch={handleLaunchClick}
                  onEdit={handleEditClick}
                  onToggleFavorite={() => {
                    /* Favorites are managed from LibraryPage in Phase 2. */
                  }}
                  isLaunching={launchingName === card.name}
                />
              ))}
            </ul>
          )}
        </div>

        <footer className="crosshook-modal__footer">
          {exportError !== null ? (
            <p className="crosshook-collection-modal__export-error" role="alert">
              {exportError}
            </p>
          ) : null}
          <div className="crosshook-modal__footer-actions">
            <button
              type="button"
              className="crosshook-button crosshook-button--ghost"
              onClick={() => void handleExportPreset()}
            >
              Export Preset
            </button>
            {deleteConfirm ? (
              <button
                type="button"
                className="crosshook-button crosshook-button--ghost"
                onClick={() => setDeleteConfirm(false)}
              >
                Cancel
              </button>
            ) : null}
            <button
              type="button"
              className="crosshook-button crosshook-button--danger"
              onClick={() => {
                if (!deleteConfirm) {
                  setDeleteConfirm(true);
                  return;
                }
                void handleDeleteCollection();
              }}
            >
              {deleteConfirm ? 'Confirm delete' : 'Delete collection'}
            </button>
            <button type="button" className="crosshook-button" onClick={onClose}>
              Done
            </button>
          </div>
        </footer>
      </div>
    </div>,
    portalHostRef.current
  );

  return (
    <>
      {mainModal}
      <BrowserDevPresetExplainerModal
        mode="export"
        open={exportExplainerOpen}
        onClose={() => setExportExplainerOpen(false)}
        onContinue={() => void handleExportExplainerContinue()}
      />
    </>
  );
}
