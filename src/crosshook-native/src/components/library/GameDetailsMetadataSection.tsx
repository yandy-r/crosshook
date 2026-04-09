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
  const trimmedId = steamAppId.trim();
  const hasAppId = /^\d+$/.test(trimmedId);

  const metaState = sectionStateFromMetadata(hasAppId, meta.loading, meta.isUnavailable, meta.state);
  const shortDesc = meta.appDetails?.short_description?.trim() ?? '';
  const steamName = meta.appDetails?.name?.trim() ?? '';
  const genres: SteamGenre[] = meta.appDetails?.genres ?? [];

  return (
    <section
      className="crosshook-game-details-modal__section crosshook-game-details-modal__section--card"
      aria-labelledby="crosshook-game-details-metadata-heading"
    >
      <h3 id="crosshook-game-details-metadata-heading" className="crosshook-game-details-modal__section-title">
        Store metadata
      </h3>
      {!hasAppId ? (
        <p className="crosshook-game-details-modal__muted">
          No Steam App ID on this profile. Metadata lookup is unavailable.
        </p>
      ) : null}
      {hasAppId && metaState === 'loading' ? (
        <p className="crosshook-game-details-modal__muted">Loading Steam metadata…</p>
      ) : null}
      {hasAppId && metaState === 'unavailable' && !meta.loading ? (
        <p className="crosshook-game-details-modal__muted">Steam metadata is unavailable (offline or not cached).</p>
      ) : null}
      {hasAppId && meta.isStale ? (
        <p className="crosshook-game-details-modal__stale">Showing cached metadata; values may be stale.</p>
      ) : null}
      {hasAppId && metaState === 'ready' && steamName ? (
        <p className="crosshook-game-details-modal__text">
          <span className="crosshook-game-details-modal__label">Steam title: </span>
          {steamName}
        </p>
      ) : null}
      {hasAppId && metaState === 'ready' && genres.length > 0 ? (
        <div className="crosshook-game-details-modal__genre-row" aria-label="Genres">
          {genres.map((genre) => (
            <span key={genre.id} className="crosshook-game-details-modal__genre-chip">
              {genre.description}
            </span>
          ))}
        </div>
      ) : null}
      {hasAppId && metaState === 'ready' && shortDesc ? (
        <p className="crosshook-game-details-modal__text crosshook-game-details-modal__text--desc">{shortDesc}</p>
      ) : null}
    </section>
  );
}
