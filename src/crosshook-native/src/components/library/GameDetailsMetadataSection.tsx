import { useId } from 'react';
import type { UseGameMetadataResult } from '../../hooks/useGameMetadata';
import type { GameDetailsSectionState } from '../../types/game-details-modal';
import type { SteamGenre } from '../../types/game-metadata';

export interface GameDetailsMetadataSectionProps {
  steamAppId: string;
  meta: UseGameMetadataResult;
}

function sectionStateFromMetadata(
  hasAppId: boolean,
  loading: boolean,
  unavailable: boolean,
  state: string
): GameDetailsSectionState {
  if (!hasAppId) {
    return 'unavailable';
  }
  if (loading || state === 'loading' || state === 'idle') {
    return 'loading';
  }
  if (unavailable || state === 'unavailable') {
    return 'unavailable';
  }
  return 'ready';
}

export function GameDetailsMetadataSection({ steamAppId, meta }: GameDetailsMetadataSectionProps) {
  const metadataHeadingId = useId();
  const trimmedId = steamAppId.trim();
  const hasAppId = /^\d+$/.test(trimmedId);

  const metaState = sectionStateFromMetadata(hasAppId, meta.loading, meta.isUnavailable, meta.state);
  const shortDesc = meta.appDetails?.short_description?.trim() ?? '';
  const steamName = meta.appDetails?.name?.trim() ?? '';
  const genres: SteamGenre[] = meta.appDetails?.genres ?? [];

  return (
    <section
      className="crosshook-hero-detail__section crosshook-hero-detail__section--card"
      aria-labelledby={metadataHeadingId}
    >
      <h3 id={metadataHeadingId} className="crosshook-hero-detail__section-title">
        Store metadata
      </h3>
      {!hasAppId ? (
        <p className="crosshook-hero-detail__muted">No Steam App ID on this profile. Metadata lookup is unavailable.</p>
      ) : null}
      {hasAppId && metaState === 'loading' ? (
        <p className="crosshook-hero-detail__muted">Loading Steam metadata…</p>
      ) : null}
      {hasAppId && metaState === 'unavailable' && !meta.loading ? (
        <p className="crosshook-hero-detail__muted">Steam metadata is unavailable (offline or not cached).</p>
      ) : null}
      {hasAppId && meta.isStale ? (
        <p className="crosshook-hero-detail__stale">Showing cached metadata; values may be stale.</p>
      ) : null}
      {hasAppId && metaState === 'ready' && steamName ? (
        <p className="crosshook-hero-detail__text">
          <span className="crosshook-hero-detail__label">Steam title: </span>
          {steamName}
        </p>
      ) : null}
      {hasAppId && metaState === 'ready' && genres.length > 0 ? (
        <ul aria-label="Genres" className="crosshook-hero-detail__genre-row">
          {genres.map((genre) => (
            <li key={genre.id} className="crosshook-hero-detail__genre-chip">
              {genre.description}
            </li>
          ))}
        </ul>
      ) : null}
      {hasAppId && metaState === 'ready' && shortDesc ? (
        <p className="crosshook-hero-detail__text crosshook-hero-detail__text--desc">{shortDesc}</p>
      ) : null}
    </section>
  );
}
