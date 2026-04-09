import { createPortal } from 'react-dom';
import {
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type MouseEvent,
} from 'react';

import type { LibraryCardData } from '../../types/library';
import type { EnrichedProfileHealthReport } from '../../types/health';
import type { OfflineReadinessReport } from '../../types';
import { useGameDetailsProfile } from '../../hooks/useGameDetailsProfile';
import { useGameCoverArt } from '../../hooks/useGameCoverArt';
import { useGameMetadata } from '../../hooks/useGameMetadata';
import { resolveLaunchMethod } from '../../utils/launch';
import { effectiveGameArtPath } from '../../utils/profile-art';
import { gameDetailsEditThenNavigate, gameDetailsLaunchThenNavigate } from './game-details-actions';
import { GameDetailsCompatibilitySection } from './GameDetailsCompatibilitySection';
import { GameDetailsHealthSection } from './GameDetailsHealthSection';
import { GameDetailsMetadataSection } from './GameDetailsMetadataSection';

import './GameDetailsModal.css';

// TODO: migrate to useFocusTrap from src/hooks/useFocusTrap.ts.
//       This file intentionally keeps a private focus-trap copy to avoid risk in PR #186.
//       See lib/focus-utils.ts for the shared FOCUSABLE_SELECTOR.
const FOCUSABLE_SELECTOR = [
  'a[href]',
  'button:not([disabled])',
  'input:not([disabled]):not([type="hidden"])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
  '[contenteditable="true"]',
].join(', ');

function getFocusableElements(container: HTMLElement) {
  return Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(
    (element) => !element.hasAttribute('disabled') && element.tabIndex >= 0 && element.getClientRects().length > 0,
  );
}

function focusElement(element: HTMLElement | null) {
  if (!element) {
    return false;
  }
  element.focus({ preventScroll: true });
  return document.activeElement === element;
}

function displayPath(value: string | null | undefined): string {
  const trimmed = value?.trim();
  return trimmed && trimmed.length > 0 ? trimmed : 'Not set';
}

/**
 * Hero precedence: custom background → SteamGridDB background → SteamGridDB hero → Steam header_image → none.
 */
function resolveGameDetailsHero(args: {
  customBgPath?: string;
  bg: { url: string | null; loading: boolean };
  hero: { url: string | null; loading: boolean };
  headerImage: string | null;
  metaLoading: boolean;
}): { url: string | null; showSkeleton: boolean } {
  const custom = args.customBgPath?.trim();
  if (custom) {
    return { url: args.bg.url, showSkeleton: args.bg.loading };
  }
  if (args.bg.loading) {
    return { url: null, showSkeleton: true };
  }
  if (args.bg.url) {
    return { url: args.bg.url, showSkeleton: false };
  }
  if (args.hero.loading) {
    return { url: null, showSkeleton: true };
  }
  if (args.hero.url) {
    return { url: args.hero.url, showSkeleton: false };
  }
  if (args.metaLoading) {
    return { url: null, showSkeleton: true };
  }
  if (args.headerImage) {
    return { url: args.headerImage, showSkeleton: false };
  }
  return { url: null, showSkeleton: false };
}

export interface GameDetailsModalProps {
  open: boolean;
  summary: LibraryCardData | null;
  onClose: () => void;
  healthByName: Partial<Record<string, EnrichedProfileHealthReport>>;
  healthLoading: boolean;
  offlineReportFor: (profileName: string) => OfflineReadinessReport | undefined;
  offlineError: string | null;
  onLaunch: (name: string) => void | Promise<void>;
  onEdit: (name: string) => void | Promise<void>;
  onToggleFavorite: (name: string, current: boolean) => void;
  launchingName?: string;
}

export function GameDetailsModal({
  open,
  summary,
  onClose,
  healthByName,
  healthLoading,
  offlineReportFor,
  offlineError,
  onLaunch,
  onEdit,
  onToggleFavorite,
  launchingName,
}: GameDetailsModalProps) {
  const portalHostRef = useRef<HTMLElement | null>(null);
  const surfaceRef = useRef<HTMLDivElement | null>(null);
  const closeButtonRef = useRef<HTMLButtonElement | null>(null);
  const headingRef = useRef<HTMLHeadingElement | null>(null);
  const previouslyFocusedRef = useRef<HTMLElement | null>(null);
  const bodyStyleRef = useRef<string>('');
  const hiddenNodesRef = useRef<Array<{ element: HTMLElement; inert: boolean; ariaHidden: string | null }>>([]);
  const titleId = useId();
  const descriptionId = useId();
  const [isMounted, setIsMounted] = useState(false);

  const profileName = summary?.name ?? null;
  const { loadState, profile, errorMessage } = useGameDetailsProfile(profileName, open && summary !== null);

  const steamAppIdForHooks = summary?.steamAppId?.trim() ?? '';
  const hasNumericAppId = /^\d+$/.test(steamAppIdForHooks);
  const appIdForArt = hasNumericAppId ? steamAppIdForHooks : undefined;

  const customBgPath = effectiveGameArtPath(profile, 'custom_background_art_path');
  const customPortraitPath =
    loadState === 'ready' && profile
      ? effectiveGameArtPath(profile, 'custom_portrait_art_path') ?? summary?.customPortraitArtPath
      : summary?.customPortraitArtPath;

  const meta = useGameMetadata(appIdForArt);
  const backgroundArt = useGameCoverArt(appIdForArt, customBgPath, 'background');
  const heroGridArt = useGameCoverArt(appIdForArt, undefined, 'hero');
  const portraitArt = useGameCoverArt(appIdForArt, customPortraitPath, 'portrait');

  const headerImage = meta.appDetails?.header_image?.trim() || null;
  const metaLoading =
    Boolean(hasNumericAppId) && (meta.loading || meta.state === 'idle' || meta.state === 'loading');

  const heroResolved = useMemo(
    () =>
      resolveGameDetailsHero({
        customBgPath,
        bg: { url: backgroundArt.coverArtUrl, loading: backgroundArt.loading },
        hero: { url: heroGridArt.coverArtUrl, loading: heroGridArt.loading },
        headerImage,
        metaLoading,
      }),
    [
      customBgPath,
      backgroundArt.coverArtUrl,
      backgroundArt.loading,
      heroGridArt.coverArtUrl,
      heroGridArt.loading,
      headerImage,
      metaLoading,
    ],
  );

  const [heroImgBroken, setHeroImgBroken] = useState(false);
  const [portraitImgBroken, setPortraitImgBroken] = useState(false);

  useEffect(() => {
    setHeroImgBroken(false);
  }, [heroResolved.url]);

  useEffect(() => {
    setPortraitImgBroken(false);
  }, [portraitArt.coverArtUrl]);

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
    if (!open || !summary || typeof document === 'undefined') {
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
  }, [open, summary]);

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

  if (!open || !isMounted || !portalHostRef.current || !summary) {
    return null;
  }

  const displayName = summary.gameName || summary.name;
  const steamAppId = steamAppIdForHooks;
  const methodLabel = profile ? resolveLaunchMethod(profile) : null;
  const healthReport = healthByName[summary.name];
  const offlineReport = offlineReportFor(summary.name);
  const isLaunchingThis = launchingName === summary.name;
  const gamePath = displayPath(profile?.game?.executable_path);
  const trainerPath = displayPath(profile?.trainer?.path);
  const prefixPath = displayPath(profile?.runtime?.prefix_path);

  return createPortal(
    <div className="crosshook-modal" role="presentation">
      <div className="crosshook-modal__backdrop" aria-hidden="true" onMouseDown={handleBackdropMouseDown} />
      <div
        ref={surfaceRef}
        className="crosshook-modal__surface crosshook-panel crosshook-focus-scope crosshook-game-details-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={descriptionId}
        data-crosshook-focus-root="modal"
        onKeyDown={handleKeyDown}
      >
        <header className="crosshook-modal__header">
          <div className="crosshook-modal__heading-block">
            <div className="crosshook-heading-eyebrow">Library</div>
            <h2 ref={headingRef} id={titleId} className="crosshook-modal__title" tabIndex={-1}>
              {displayName}
            </h2>
            <p id={descriptionId} className="crosshook-modal__description">
              Read-only details for profile <span className="crosshook-game-details-modal__mono">{summary.name}</span>.
            </p>
          </div>
          <div className="crosshook-modal__header-actions">
            <button
              ref={closeButtonRef}
              type="button"
              className="crosshook-button crosshook-button--ghost crosshook-modal__close"
              onClick={onClose}
              aria-label="Close game details dialog"
              data-crosshook-modal-close
            >
              Close
            </button>
          </div>
        </header>

        <section className="crosshook-modal__summary" aria-label="Profile summary">
          <div className="crosshook-modal__summary-item">
            <div className="crosshook-modal__summary-label">Profile</div>
            <div className="crosshook-modal__summary-value crosshook-modal__summary-value--mono">{summary.name}</div>
          </div>
          <div className="crosshook-modal__summary-item">
            <div className="crosshook-modal__summary-label">Steam App ID</div>
            <div className="crosshook-modal__summary-value crosshook-modal__summary-value--mono">
              {steamAppId || 'Not set'}
            </div>
          </div>
          <div className="crosshook-modal__summary-item">
            <div className="crosshook-modal__summary-label">Favorite</div>
            <div className="crosshook-modal__summary-value">{summary.isFavorite ? 'Yes' : 'No'}</div>
          </div>
          <div className="crosshook-modal__summary-item">
            <div className="crosshook-modal__summary-label">Launch method</div>
            <div className="crosshook-modal__summary-value crosshook-modal__summary-value--mono">
              {loadState === 'loading' ? 'Loading…' : methodLabel ?? '—'}
            </div>
          </div>
        </section>

        <div className="crosshook-modal__body crosshook-game-details-modal__body">
          <div className="crosshook-game-details-modal__hero" aria-hidden="true">
            {heroResolved.showSkeleton ? (
              <div className="crosshook-game-details-modal__hero-skeleton crosshook-skeleton" />
            ) : null}
            {heroResolved.url && !heroImgBroken && !heroResolved.showSkeleton ? (
              <img
                className="crosshook-game-details-modal__hero-img"
                src={heroResolved.url}
                alt=""
                onError={() => setHeroImgBroken(true)}
              />
            ) : null}
            <div className="crosshook-game-details-modal__hero-gradient" />
          </div>

          <div className="crosshook-game-details-modal__below-hero">
            <div className="crosshook-game-details-modal__layout">
              <aside className="crosshook-game-details-modal__media-rail" aria-label="Portrait artwork">
                <div className="crosshook-game-details-modal__portrait-wrap">
                  {portraitArt.loading ? (
                    <div
                      className="crosshook-game-details-modal__portrait crosshook-game-details-modal__portrait--skeleton crosshook-skeleton"
                      aria-hidden
                    />
                  ) : portraitArt.coverArtUrl && !portraitImgBroken ? (
                    <img
                      className="crosshook-game-details-modal__portrait"
                      src={portraitArt.coverArtUrl}
                      alt={`${displayName} portrait art`}
                      onError={() => setPortraitImgBroken(true)}
                    />
                  ) : (
                    <div className="crosshook-game-details-modal__portrait-fallback" aria-hidden>
                      {displayName.slice(0, 2).toUpperCase()}
                    </div>
                  )}
                </div>
              </aside>

              <div className="crosshook-game-details-modal__main">
                {loadState === 'loading' ? (
                  <p className="crosshook-game-details-modal__muted">Loading profile details…</p>
                ) : null}
                {loadState === 'error' ? (
                  <p className="crosshook-game-details-modal__warn">{errorMessage ?? 'Failed to load profile.'}</p>
                ) : null}
                {profile && loadState === 'ready' ? (
                  <section
                    className="crosshook-game-details-modal__section crosshook-game-details-modal__section--card"
                    aria-label="Executable paths"
                  >
                    <h3 className="crosshook-game-details-modal__section-title">Paths</h3>
                    <p className="crosshook-game-details-modal__text">
                      <span className="crosshook-game-details-modal__label">Game: </span>
                      <span className="crosshook-game-details-modal__mono">{gamePath}</span>
                    </p>
                    <p className="crosshook-game-details-modal__text">
                      <span className="crosshook-game-details-modal__label">Trainer: </span>
                      <span className="crosshook-game-details-modal__mono">{trainerPath}</span>
                    </p>
                    <p className="crosshook-game-details-modal__text">
                      <span className="crosshook-game-details-modal__label">Prefix: </span>
                      <span className="crosshook-game-details-modal__mono">{prefixPath}</span>
                    </p>
                  </section>
                ) : null}

                <GameDetailsMetadataSection steamAppId={steamAppId} meta={meta} />
                <GameDetailsCompatibilitySection steamAppId={steamAppId} />
                <GameDetailsHealthSection
                  profileName={summary.name}
                  healthReport={healthReport}
                  healthLoading={healthLoading}
                  offlineReport={offlineReport}
                  offlineError={offlineError}
                />
              </div>
            </div>
          </div>
        </div>

        <footer className="crosshook-modal__footer">
          <div className="crosshook-modal__footer-copy">Quick actions use the same flows as library cards.</div>
          <div className="crosshook-modal__footer-actions">
            <button
              type="button"
              className="crosshook-button"
              disabled={isLaunchingThis}
              onClick={() => gameDetailsLaunchThenNavigate(summary.name, onLaunch, onClose)}
            >
              {isLaunchingThis ? 'Launching…' : 'Launch'}
            </button>
            <button
              type="button"
              className="crosshook-button crosshook-button--secondary"
              aria-pressed={summary.isFavorite}
              onClick={() => onToggleFavorite(summary.name, summary.isFavorite)}
            >
              {summary.isFavorite ? 'Unfavorite' : 'Favorite'}
            </button>
            <button
              type="button"
              className="crosshook-button crosshook-button--secondary"
              onClick={() => gameDetailsEditThenNavigate(summary.name, onEdit, onClose)}
            >
              Edit profile
            </button>
          </div>
        </footer>
      </div>
    </div>,
    portalHostRef.current,
  );
}

export default GameDetailsModal;
