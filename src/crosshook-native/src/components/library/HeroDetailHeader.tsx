import type { GameProfile } from '@/types';
import type { LibraryCardData } from '@/types/library';
import { gameDetailsEditThenNavigate, gameDetailsLaunchThenNavigate } from './game-details-actions';
import { displayPath } from './hero-detail-model';

export interface HeroDetailHeaderProps {
  summary: LibraryCardData;
  displayName: string;
  profile: GameProfile | null;
  loadState: 'idle' | 'loading' | 'ready' | 'error';
  profileError: string | null;
  methodLabel: string | null;
  heroResolved: { url: string | null; showSkeleton: boolean };
  portraitArt: { coverArtUrl: string | null; loading: boolean };
  heroImgBroken: boolean;
  setHeroImgBroken: (broken: boolean) => void;
  portraitImgBroken: boolean;
  setPortraitImgBroken: (broken: boolean) => void;
  launchingName?: string;
  onBack: () => void;
  onLaunch: (name: string) => void | Promise<void>;
  onEdit: (name: string) => void | Promise<void>;
  onToggleFavorite: (name: string, current: boolean) => void;
}

export function HeroDetailHeader({
  summary,
  displayName,
  profile,
  loadState,
  profileError,
  methodLabel,
  heroResolved,
  portraitArt,
  heroImgBroken,
  setHeroImgBroken,
  portraitImgBroken,
  setPortraitImgBroken,
  launchingName,
  onBack,
  onLaunch,
  onEdit,
  onToggleFavorite,
}: HeroDetailHeaderProps) {
  const steamAppId = summary.steamAppId?.trim() ?? '';
  const isLaunchingThis = launchingName === summary.name;
  const gamePath = displayPath(profile?.game?.executable_path);
  const trainerPath = displayPath(profile?.trainer?.path);
  const prefixPath = displayPath(profile?.runtime?.prefix_path);

  return (
    <div className="crosshook-hero-detail__header-shell">
      <div className="crosshook-hero-detail__top-bar">
        <button type="button" className="crosshook-button crosshook-button--ghost" onClick={onBack}>
          Back
        </button>
        <div className="crosshook-hero-detail__quick-actions">
          <button
            type="button"
            className="crosshook-button crosshook-button--small"
            disabled={isLaunchingThis}
            onClick={() => gameDetailsLaunchThenNavigate(summary.name, onLaunch, onBack)}
          >
            {isLaunchingThis ? 'Launching…' : 'Launch'}
          </button>
          <button
            type="button"
            className="crosshook-button crosshook-button--small crosshook-button--secondary"
            aria-pressed={summary.isFavorite}
            onClick={() => onToggleFavorite(summary.name, summary.isFavorite)}
          >
            {summary.isFavorite ? 'Unfavorite' : 'Favorite'}
          </button>
          <button
            type="button"
            className="crosshook-button crosshook-button--small crosshook-button--secondary"
            onClick={() => gameDetailsEditThenNavigate(summary.name, onEdit, onBack)}
          >
            Edit profile
          </button>
        </div>
      </div>

      <div className="crosshook-hero-detail__hero" aria-hidden="true">
        {heroResolved.showSkeleton ? <div className="crosshook-hero-detail__hero-skeleton crosshook-skeleton" /> : null}
        {heroResolved.url && !heroImgBroken && !heroResolved.showSkeleton ? (
          <img
            className="crosshook-hero-detail__hero-img"
            src={heroResolved.url}
            alt=""
            onError={() => setHeroImgBroken(true)}
          />
        ) : null}
        <div className="crosshook-hero-detail__hero-gradient" />
      </div>

      <div className="crosshook-hero-detail__below-hero">
        <div className="crosshook-hero-detail__layout">
          <aside className="crosshook-hero-detail__media-rail" aria-label="Portrait artwork">
            <div className="crosshook-hero-detail__portrait-wrap">
              {portraitArt.loading ? (
                <div
                  className="crosshook-hero-detail__portrait crosshook-hero-detail__portrait--skeleton crosshook-skeleton"
                  aria-hidden
                />
              ) : portraitArt.coverArtUrl && !portraitImgBroken ? (
                <img
                  className="crosshook-hero-detail__portrait"
                  src={portraitArt.coverArtUrl}
                  alt={`${displayName} portrait art`}
                  onError={() => setPortraitImgBroken(true)}
                />
              ) : (
                <div className="crosshook-hero-detail__portrait-fallback" aria-hidden>
                  {displayName.slice(0, 2).toUpperCase()}
                </div>
              )}
            </div>
          </aside>

          <div className="crosshook-hero-detail__main-brief">
            <div className="crosshook-hero-detail__title-block">
              <p className="crosshook-hero-detail__eyebrow">Library</p>
              <h2 className="crosshook-hero-detail__title">{displayName}</h2>
              <p className="crosshook-hero-detail__subtitle">
                Profile <span className="crosshook-hero-detail__mono">{summary.name}</span>
              </p>
            </div>
            <section className="crosshook-hero-detail__pills" aria-label="Profile summary">
              <span className="crosshook-hero-detail__pill">Steam app {steamAppId || 'Not set'}</span>
              <span className="crosshook-hero-detail__pill">Favorite: {summary.isFavorite ? 'Yes' : 'No'}</span>
              <span className="crosshook-hero-detail__pill">
                Network: {summary.networkIsolation ? 'Isolated' : 'Default'}
              </span>
              <span className="crosshook-hero-detail__pill">
                Launch method: {loadState === 'loading' ? 'Loading…' : (methodLabel ?? '—')}
              </span>
            </section>
            {loadState === 'loading' ? <p className="crosshook-hero-detail__muted">Loading profile details…</p> : null}
            {loadState === 'error' ? (
              <p className="crosshook-hero-detail__warn">{profileError ?? 'Failed to load profile.'}</p>
            ) : null}
            {profile && loadState === 'ready' ? (
              <section
                className="crosshook-hero-detail__section crosshook-hero-detail__section--card"
                aria-label="Executable paths"
              >
                <h3 className="crosshook-hero-detail__section-title">Paths</h3>
                <p className="crosshook-hero-detail__text">
                  <span className="crosshook-hero-detail__label">Game: </span>
                  <span className="crosshook-hero-detail__mono">{gamePath}</span>
                </p>
                <p className="crosshook-hero-detail__text">
                  <span className="crosshook-hero-detail__label">Trainer: </span>
                  <span className="crosshook-hero-detail__mono">{trainerPath}</span>
                </p>
                <p className="crosshook-hero-detail__text">
                  <span className="crosshook-hero-detail__label">Prefix: </span>
                  <span className="crosshook-hero-detail__mono">{prefixPath}</span>
                </p>
              </section>
            ) : null}
          </div>
        </div>
      </div>
    </div>
  );
}
