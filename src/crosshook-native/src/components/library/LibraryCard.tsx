import { useEffect, useRef, useState, type KeyboardEvent } from 'react';
import type { LibraryCardData } from '../../types/library';
import { useGameCoverArt } from '../../hooks/useGameCoverArt';
import type { LibraryOpenDetailsHandler } from './library-card-interactions';

interface LibraryCardProps {
  profile: LibraryCardData;
  isSelected?: boolean;
  onOpenDetails: LibraryOpenDetailsHandler;
  onLaunch: (name: string) => void;
  onEdit: (name: string) => void;
  onToggleFavorite: (name: string, current: boolean) => void;
  isLaunching?: boolean;
  onContextMenu?: (
    position: { x: number; y: number },
    profileName: string,
    restoreFocusTo: HTMLElement
  ) => void;
}

/** Keyboard shortcut to open context menu: Shift+F10 or the ContextMenu key. */
function isContextMenuKey(event: KeyboardEvent<HTMLDivElement>): boolean {
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
  onLaunch,
  onEdit,
  onToggleFavorite,
  isLaunching,
  onContextMenu,
}: LibraryCardProps) {
  // IntersectionObserver: only fetch cover art when card enters viewport
  const [visible, setVisible] = useState(false);
  const cardRef = useRef<HTMLDivElement>(null);

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
    'portrait',
  );

  const [imgFailed, setImgFailed] = useState(false);
  useEffect(() => setImgFailed(false), [coverArtUrl]);

  const hasMedia = !!(coverArtUrl && !imgFailed);
  const showTitle = !hasMedia && !loading;

  const cardClass = [
    'crosshook-library-card',
    isSelected && 'crosshook-library-card--selected',
  ].filter(Boolean).join(' ');

  const displayName = profile.gameName || profile.name;

  function handleOpenDetailsClick() {
    onOpenDetails(profile.name);
  }

  return (
    <div
      ref={cardRef}
      className={cardClass}
      role="listitem"
      tabIndex={0}
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
          ? (e: KeyboardEvent<HTMLDivElement>) => {
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
              }
            }
          : undefined
      }
    >
      <button
        type="button"
        className="crosshook-library-card__details-hitbox"
        aria-label={`View details for ${displayName}`}
        onClick={handleOpenDetailsClick}
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
        <div className="crosshook-library-card__fallback">
          {getInitials(profile.gameName, profile.name)}
        </div>
      )}

      {/* Gradient scrim */}
      <div className="crosshook-library-card__scrim" />

      {/* Favorite badge (persistent when favorited) */}
      {profile.isFavorite && (
        <div className="crosshook-library-card__favorite-badge" aria-label="Favorited">
          <svg width="16" height="16" viewBox="0 0 20 20" fill="currentColor">
            <path d="M10 17.5S2 13 2 7.5A4 4 0 0 1 10 5.1 4 4 0 0 1 18 7.5C18 13 10 17.5 10 17.5z" />
          </svg>
        </div>
      )}

      {/* Footer with title and actions */}
      <div className="crosshook-library-card__footer">
        {showTitle && <span className="crosshook-library-card__title">{displayName}</span>}
        <div className="crosshook-library-card__actions">
          <button
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
            className="crosshook-library-card__btn--glass"
            aria-label={profile.isFavorite ? `Unfavorite ${displayName}` : `Favorite ${displayName}`}
            aria-pressed={profile.isFavorite}
            onClick={(e) => {
              e.stopPropagation();
              onToggleFavorite(profile.name, profile.isFavorite);
            }}
          >
            <svg width="14" height="14" viewBox="0 0 20 20" fill={profile.isFavorite ? 'currentColor' : 'none'} stroke="currentColor" strokeWidth="1.5">
              <path d="M10 17.5S2 13 2 7.5A4 4 0 0 1 10 5.1 4 4 0 0 1 18 7.5C18 13 10 17.5 10 17.5z" />
            </svg>
          </button>
          <button
            className="crosshook-library-card__btn--glass"
            aria-label={`Edit ${displayName}`}
            onClick={(e) => {
              e.stopPropagation();
              onEdit(profile.name);
            }}
          >
            <svg width="14" height="14" viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
              <path d="M14.5 2.5l3 3L6 17H3v-3z" />
            </svg>
          </button>
        </div>
      </div>
    </div>
  );
}

export default LibraryCard;
