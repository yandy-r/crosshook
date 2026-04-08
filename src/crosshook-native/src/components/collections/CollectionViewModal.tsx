import { createPortal } from 'react-dom';
import {
  useCallback,
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type MouseEvent,
} from 'react';

import type { CollectionRow } from '@/types/collections';
import type { LibraryCardData } from '@/types/library';
import { useCollectionMembers } from '@/hooks/useCollectionMembers';
import { useCollections } from '@/hooks/useCollections';
import { useLibraryProfiles } from '@/hooks/useLibraryProfiles';
import { useLibrarySummaries } from '@/hooks/useLibrarySummaries';
import { useProfileContext } from '@/context/ProfileContext';
import { LibraryCard } from '@/components/library/LibraryCard';
import { gameDetailsEditThenNavigate, gameDetailsLaunchThenNavigate } from '@/components/library/game-details-actions';
import { getFocusableElements } from '@/lib/focus-utils';

import './CollectionViewModal.css';

function focusElement(element: HTMLElement | null) {
  if (!element) {
    return false;
  }
  element.focus({ preventScroll: true });
  return document.activeElement === element;
}

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
}: CollectionViewModalProps) {
  const titleId = useId();
  const descriptionId = useId();
  const surfaceRef = useRef<HTMLDivElement | null>(null);
  const headingRef = useRef<HTMLHeadingElement | null>(null);
  const closeButtonRef = useRef<HTMLButtonElement | null>(null);
  const portalHostRef = useRef<HTMLElement | null>(null);
  const previouslyFocusedRef = useRef<HTMLElement | null>(null);
  const bodyStyleRef = useRef<string>('');
  const hiddenNodesRef = useRef<Array<{ element: HTMLElement; inert: boolean; ariaHidden: string | null }>>([]);
  const [isMounted, setIsMounted] = useState(false);

  const { collections, deleteCollection } = useCollections();
  const { memberNames, loading: membersLoading } = useCollectionMembers(open ? collectionId : null);
  const { profiles, favoriteProfiles, selectedProfile } = useProfileContext();
  const { summaries } = useLibrarySummaries(profiles, favoriteProfiles);

  const [searchQuery, setSearchQuery] = useState('');
  const [deleteConfirm, setDeleteConfirm] = useState(false);

  useEffect(() => {
    setSearchQuery('');
  }, [open, collectionId]);

  useEffect(() => {
    if (!open) {
      setDeleteConfirm(false);
    }
  }, [open]);

  const collection = useMemo<CollectionRow | null>(
    () => collections.find((c) => c.collection_id === collectionId) ?? null,
    [collections, collectionId]
  );

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
    if (typeof document === 'undefined') {
      return;
    }
    const host = document.createElement('div');
    host.className = 'crosshook-modal-portal';
    portalHostRef.current = host;
    document.body.appendChild(host);
    setIsMounted(true);
    return () => {
      host.remove();
      portalHostRef.current = null;
      setIsMounted(false);
    };
  }, []);

  useEffect(() => {
    if (!open || !collection || typeof document === 'undefined') {
      return;
    }
    const { body } = document;
    const portalHost = portalHostRef.current;
    if (!portalHost) {
      return;
    }

    previouslyFocusedRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    bodyStyleRef.current = body.style.overflow;
    body.style.overflow = 'hidden';
    body.classList.add('crosshook-modal-open');

    hiddenNodesRef.current = Array.from(body.children)
      .filter((child): child is HTMLElement => child instanceof HTMLElement && child !== portalHost)
      .map((element) => {
        const inertState = (element as HTMLElement & { inert?: boolean }).inert ?? false;
        const ariaHidden = element.getAttribute('aria-hidden');
        (element as HTMLElement & { inert?: boolean }).inert = true;
        element.setAttribute('aria-hidden', 'true');
        return { element, inert: inertState, ariaHidden };
      });

    const focusTarget = headingRef.current ?? closeButtonRef.current ?? null;
    const frame = window.requestAnimationFrame(() => {
      if (focusElement(focusTarget)) {
        return;
      }
      const focusable = surfaceRef.current ? getFocusableElements(surfaceRef.current) : [];
      if (focusable.length > 0) {
        focusElement(focusable[0]);
      }
    });

    return () => {
      window.cancelAnimationFrame(frame);
      body.style.overflow = bodyStyleRef.current;
      body.classList.remove('crosshook-modal-open');
      for (const { element, inert, ariaHidden } of hiddenNodesRef.current) {
        (element as HTMLElement & { inert?: boolean }).inert = inert;
        if (ariaHidden === null) {
          element.removeAttribute('aria-hidden');
        } else {
          element.setAttribute('aria-hidden', ariaHidden);
        }
      }
      hiddenNodesRef.current = [];
      const restoreTarget = previouslyFocusedRef.current;
      if (restoreTarget && restoreTarget.isConnected) {
        focusElement(restoreTarget);
      }
      previouslyFocusedRef.current = null;
    };
  }, [open, collection]);

  function handleKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === 'Escape') {
      event.stopPropagation();
      event.preventDefault();
      onClose();
      return;
    }
    if (event.key !== 'Tab') {
      return;
    }
    const container = surfaceRef.current;
    if (!container) {
      return;
    }
    const focusable = getFocusableElements(container);
    if (focusable.length === 0) {
      event.preventDefault();
      return;
    }
    const currentIndex = focusable.indexOf(document.activeElement as HTMLElement);
    const lastIndex = focusable.length - 1;
    if (event.shiftKey) {
      if (currentIndex <= 0) {
        event.preventDefault();
        focusElement(focusable[lastIndex]);
      }
      return;
    }
    if (currentIndex === -1 || currentIndex === lastIndex) {
      event.preventDefault();
      focusElement(focusable[0]);
    }
  }

  function handleBackdropMouseDown(event: MouseEvent<HTMLDivElement>) {
    if (event.target !== event.currentTarget) {
      return;
    }
    onClose();
  }

  if (!open || !collection || !isMounted || !portalHostRef.current) {
    return null;
  }

  return createPortal(
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
              ref={closeButtonRef}
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
            <div id={`${titleId}-results`} className="crosshook-collection-modal__grid" role="list">
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
            </div>
          )}
        </div>

        <footer className="crosshook-modal__footer">
          <div className="crosshook-modal__footer-actions">
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
}
