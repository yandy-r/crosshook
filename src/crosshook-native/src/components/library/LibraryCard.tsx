import { type KeyboardEvent, useEffect, useRef, useState } from 'react';
import { useGameCoverArt } from '../../hooks/useGameCoverArt';
import type { LibraryCardData } from '../../types/library';
import type { LibraryOpenDetailsHandler } from './library-card-interactions';
import { useLibraryHitboxClicks } from './useLibraryHitboxClicks';

interface LibraryCardProps {
  profile: LibraryCardData;
  isSelected?: boolean;
  onOpenDetails: LibraryOpenDetailsHandler;
  /**
   * When set, a single click on the card hit area (`handleHitboxClick`) calls `onSelect` to choose the
   * game for the inspector. Opening full details is not from double-click; use the separate
   * “open details” control (`crosshook-library-card__open-details`) when this prop is provided.
   */
  onSelect?: (name: string) => void;
  onLaunch: (name: string) => void;
  onEdit: (name: string) => void;
  onToggleFavorite: (name: string, current: boolean) => void;
  isLaunching?: boolean;
  onContextMenu?: (position: { x: number; y: number }, profileName: string, restoreFocusTo: HTMLElement) => void;
}

/** Keyboard shortcut to open context menu: Shift+F10 or the ContextMenu key. */
function isContextMenuKey(event: KeyboardEvent<HTMLLIElement>): boolean {
  return event.key === 'ContextMenu' || (event.key === 'F10' && event.shiftKey);
}

function getInitials(gameName: string, name: string): string {
  const source = gameName || name;
  return source.slice(0, 2).toUpperCase();
}

export function LibraryCard({
  profile,
  isSelected,
  onOpenDetails,
  onSelect,
  onLaunch,
  onEdit,
  onToggleFavorite,
  isLaunching,
  onContextMenu,
}: LibraryCardProps) {
  // IntersectionObserver: only fetch cover art when card enters viewport
  const [visible, setVisible] = useState(false);
  const cardRef = useRef<HTMLLIElement>(null);

  useEffect(() => {
    const el = cardRef.current;
    if (!el) return;
    const obs = new IntersectionObserver(([entry]) => {
      if (entry.isIntersecting) {
        setVisible(true);
        obs.disconnect();
      }
    });
    obs.observe(el);
    return () => obs.disconnect();
  }, []);

  const { coverArtUrl, loading } = useGameCoverArt(
    visible ? profile.steamAppId : undefined,
    profile.customPortraitArtPath,
    'portrait'
  );

  const [imgFailed, setImgFailed] = useState(false);
  useEffect(() => setImgFailed(false), []);

  const hasMedia = !!(coverArtUrl && !imgFailed);
  const showTitle = !hasMedia && !loading;

  const cardClass = ['crosshook-library-card', isSelected && 'crosshook-library-card--selected']
    .filter(Boolean)
    .join(' ');

  const displayName = profile.gameName || profile.name;

  function handleOpenDetailsClick() {
    onOpenDetails(profile.name);
  }

  const { handleHitboxClick, handleHitboxDoubleClick } = useLibraryHitboxClicks({
    profileName: profile.name,
    onOpenDetails,
    onSelect,
  });

  return (
    <li
      ref={cardRef}
      className={cardClass}
      tabIndex={onContextMenu ? 0 : undefined}
      onContextMenu={
        onContextMenu
          ? (e) => {
              e.preventDefault();
              const el = cardRef.current;
              if (el) {
                onContextMenu({ x: e.clientX, y: e.clientY }, profile.name, el);
              }
            }
          : undefined
      }
      onKeyDown={
        onContextMenu
          ? (e: KeyboardEvent<HTMLLIElement>) => {
              if (isContextMenuKey(e)) {
                e.preventDefault();
                e.stopPropagation();
                const rect = cardRef.current?.getBoundingClientRect();
                const x = rect ? rect.left + rect.width / 2 : 0;
                const y = rect ? rect.top + rect.height / 2 : 0;
                const el = cardRef.current;
                if (el) {
                  onContextMenu({ x, y }, profile.name, el);
                }
                return;
              }
              if (
                e.key === 'Enter' &&
                !e.shiftKey &&
                !e.ctrlKey &&
                !e.altKey &&
                !e.metaKey &&
                !e.repeat &&
                e.target === e.currentTarget
              ) {
                e.preventDefault();
                onOpenDetails(profile.name);
              }
            }
          : undefined
      }
    >
      <button
        type="button"
        className="crosshook-library-card__details-hitbox"
        aria-label={onSelect ? `Select ${displayName}` : `View details for ${displayName}`}
        onClick={handleHitboxClick}
        onDoubleClick={handleHitboxDoubleClick}
      />
      {/* Cover image / skeleton / fallback */}
      {loading ? (
        <div className="crosshook-library-card__image crosshook-skeleton" />
      ) : hasMedia ? (
        <img
          className="crosshook-library-card__image"
          src={coverArtUrl}
          alt={displayName}
          loading="lazy"
          onError={() => setImgFailed(true)}
        />
      ) : (
        <div className="crosshook-library-card__fallback">{getInitials(profile.gameName, profile.name)}</div>
      )}

      {/* Gradient scrim */}
      <div className="crosshook-library-card__scrim" />

      <div className="crosshook-library-card__hover-reveal" aria-hidden="true" />

      <button
        type="button"
        className="crosshook-library-card__favorite-heart"
        aria-pressed={profile.isFavorite}
        aria-label={`Toggle favorite: ${displayName}`}
        onClick={(e) => {
          e.stopPropagation();
          onToggleFavorite(profile.name, profile.isFavorite);
        }}
      >
        <svg
          width="16"
          height="16"
          viewBox="0 0 20 20"
          fill={profile.isFavorite ? 'currentColor' : 'none'}
          stroke="currentColor"
          strokeWidth="1.5"
          aria-hidden="true"
        >
          <path d="M10 17.5S2 13 2 7.5A4 4 0 0 1 10 5.1 4 4 0 0 1 18 7.5C18 13 10 17.5 10 17.5z" />
        </svg>
      </button>

      {/* Footer with title and actions */}
      <div className="crosshook-library-card__footer">
        {showTitle && <span className="crosshook-library-card__title">{displayName}</span>}
        <div className="crosshook-library-card__actions">
          {onSelect ? (
            <button
              type="button"
              className="crosshook-library-card__btn--details"
              aria-label={`View details for ${displayName}`}
              onClick={(e) => {
                e.stopPropagation();
                handleOpenDetailsClick();
              }}
            >
              <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                <circle cx="8" cy="8" r="6.5" stroke="currentColor" strokeWidth="1.25" />
                <path d="M8 6.5v3M8 4.2h.01" stroke="currentColor" strokeWidth="1.25" strokeLinecap="round" />
              </svg>
            </button>
          ) : null}
          <button
            type="button"
            className="crosshook-library-card__btn--launch"
            aria-label={`Launch ${displayName}`}
            disabled={isLaunching}
            onClick={(e) => {
              e.stopPropagation();
              onLaunch(profile.name);
            }}
          >
            {isLaunching ? 'Launching...' : 'Launch'}
          </button>
          <button
            type="button"
            className="crosshook-library-card__btn--glass"
            aria-label={profile.isFavorite ? `Unfavorite ${displayName}` : `Favorite ${displayName}`}
            aria-pressed={profile.isFavorite}
            onClick={(e) => {
              e.stopPropagation();
              onToggleFavorite(profile.name, profile.isFavorite);
            }}
          >
            <svg
              width="14"
              height="14"
              viewBox="0 0 20 20"
              fill={profile.isFavorite ? 'currentColor' : 'none'}
              stroke="currentColor"
              strokeWidth="1.5"
              aria-hidden="true"
            >
              <path d="M10 17.5S2 13 2 7.5A4 4 0 0 1 10 5.1 4 4 0 0 1 18 7.5C18 13 10 17.5 10 17.5z" />
            </svg>
          </button>
          <button
            type="button"
            className="crosshook-library-card__btn--glass"
            aria-label={`Edit ${displayName}`}
            onClick={(e) => {
              e.stopPropagation();
              onEdit(profile.name);
            }}
          >
            <svg
              width="14"
              height="14"
              viewBox="0 0 20 20"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.5"
              strokeLinecap="round"
              strokeLinejoin="round"
              aria-hidden="true"
            >
              <path d="M14.5 2.5l3 3L6 17H3v-3z" />
            </svg>
          </button>
        </div>
      </div>
    </li>
  );
}

export default LibraryCard;
