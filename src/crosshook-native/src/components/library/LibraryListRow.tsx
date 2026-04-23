import { type KeyboardEvent, useEffect, useRef, useState } from 'react';
import { useGameCoverArt } from '../../hooks/useGameCoverArt';
import type { LibraryCardData } from '../../types/library';
import type { LibraryOpenDetailsHandler } from './library-card-interactions';

interface LibraryListRowProps {
  profile: LibraryCardData;
  isSelected?: boolean;
  onSelect?: (name: string) => void;
  onOpenDetails: LibraryOpenDetailsHandler;
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

export function LibraryListRow({
  profile,
  isSelected,
  onSelect,
  onOpenDetails,
  onLaunch,
  onEdit,
  onToggleFavorite,
  isLaunching,
  onContextMenu,
}: LibraryListRowProps) {
  // IntersectionObserver: only fetch cover art when row enters viewport
  const [visible, setVisible] = useState(false);
  const rowRef = useRef<HTMLLIElement>(null);

  useEffect(() => {
    const el = rowRef.current;
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

  // Use landscape/cover art for list view (wider aspect ratio fits better)
  const { coverArtUrl, loading } = useGameCoverArt(
    visible ? profile.steamAppId : undefined,
    profile.customCoverArtPath,
    'landscape'
  );

  const [imgFailed, setImgFailed] = useState(false);
  useEffect(() => setImgFailed(false), []);

  const hasMedia = !!(coverArtUrl && !imgFailed);

  const rowClass = ['crosshook-library-list-row', isSelected && 'crosshook-library-list-row--selected']
    .filter(Boolean)
    .join(' ');

  const displayName = profile.gameName || profile.name;

  function handleOpenDetailsClick() {
    onOpenDetails(profile.name);
  }

  function handleHitboxClick() {
    if (onSelect) {
      onSelect(profile.name);
      return;
    }
    handleOpenDetailsClick();
  }

  function handleHitboxDoubleClick() {
    onOpenDetails(profile.name);
  }

  return (
    <li
      ref={rowRef}
      className={rowClass}
      tabIndex={onContextMenu ? 0 : undefined}
      onContextMenu={
        onContextMenu
          ? (e) => {
              e.preventDefault();
              const el = rowRef.current;
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
                const rect = rowRef.current?.getBoundingClientRect();
                const x = rect ? rect.left + rect.width / 2 : 0;
                const y = rect ? rect.top + rect.height / 2 : 0;
                const el = rowRef.current;
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
        className="crosshook-library-list-row__details-hitbox"
        aria-label={onSelect ? `Select ${displayName}` : `View details for ${displayName}`}
        onClick={handleHitboxClick}
        onDoubleClick={(e) => {
          e.preventDefault();
          e.stopPropagation();
          handleHitboxDoubleClick();
        }}
      />

      {/* Thumbnail */}
      <div className="crosshook-library-list-row__thumbnail">
        {loading ? (
          <div className="crosshook-library-list-row__thumbnail-image crosshook-skeleton" />
        ) : hasMedia ? (
          <img
            className="crosshook-library-list-row__thumbnail-image"
            src={coverArtUrl}
            alt={displayName}
            loading="lazy"
            onError={() => setImgFailed(true)}
          />
        ) : (
          <div className="crosshook-library-list-row__thumbnail-fallback">
            {getInitials(profile.gameName, profile.name)}
          </div>
        )}
      </div>

      {/* Favorite badge */}
      {profile.isFavorite && (
        <div className="crosshook-library-list-row__favorite-badge" role="img" aria-label="Favorited">
          <svg width="14" height="14" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
            <path d="M10 17.5S2 13 2 7.5A4 4 0 0 1 10 5.1 4 4 0 0 1 18 7.5C18 13 10 17.5 10 17.5z" />
          </svg>
        </div>
      )}

      {/* Game info */}
      <div className="crosshook-library-list-row__info">
        <span className="crosshook-library-list-row__game-name">{profile.gameName || 'Unnamed Game'}</span>
        <span className="crosshook-library-list-row__profile-name">{profile.name}</span>
      </div>

      {/* Actions */}
      <div className="crosshook-library-list-row__actions">
        {onSelect ? (
          <button
            type="button"
            className="crosshook-library-list-row__btn--icon"
            aria-label={`View details for ${displayName}`}
            onClick={(e) => {
              e.stopPropagation();
              handleOpenDetailsClick();
            }}
          >
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
              <circle cx="8" cy="8" r="6.5" stroke="currentColor" strokeWidth="1.25" />
              <path d="M8 6.5v3M8 4.2h.01" stroke="currentColor" strokeWidth="1.25" strokeLinecap="round" />
            </svg>
          </button>
        ) : null}
        <button
          type="button"
          className="crosshook-library-list-row__btn--launch"
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
          className="crosshook-library-list-row__btn--icon"
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
          className="crosshook-library-list-row__btn--icon"
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
    </li>
  );
}

export default LibraryListRow;
