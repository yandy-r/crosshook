import { useGameCoverArt } from '../../hooks/useGameCoverArt';
import { useGameMetadata } from '../../hooks/useGameMetadata';
import type { GameDetailsSectionState } from '../../types/game-details-modal';

export interface GameDetailsMetadataSectionProps {
  steamAppId: string;
  customPortraitPath?: string;
  displayName: string;
}

function sectionStateFromMetadata(
  hasAppId: boolean,
  loading: boolean,
  unavailable: boolean,
  state: string,
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

export function GameDetailsMetadataSection({
  steamAppId,
  customPortraitPath,
  displayName,
}: GameDetailsMetadataSectionProps) {
  const trimmedId = steamAppId.trim();
  const hasAppId = /^\d+$/.test(trimmedId);

  const meta = useGameMetadata(hasAppId ? trimmedId : undefined);
  const { coverArtUrl, loading: coverLoading } = useGameCoverArt(
    hasAppId ? trimmedId : undefined,
    customPortraitPath,
    'portrait',
  );

  const metaState = sectionStateFromMetadata(hasAppId, meta.loading, meta.isUnavailable, meta.state);
  const shortDesc = meta.appDetails?.short_description?.trim() ?? '';
  const steamName = meta.appDetails?.name?.trim() ?? '';

  return (
    <section className="crosshook-game-details-modal__section" aria-labelledby="crosshook-game-details-metadata-heading">
      <h3 id="crosshook-game-details-metadata-heading" className="crosshook-game-details-modal__section-title">
        Store metadata
      </h3>
      {!hasAppId ? (
        <p className="crosshook-game-details-modal__muted">No Steam App ID on this profile. Metadata lookup is unavailable.</p>
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
      {hasAppId && metaState === 'ready' && shortDesc ? (
        <p className="crosshook-game-details-modal__text crosshook-game-details-modal__text--desc">{shortDesc}</p>
      ) : null}
      <div className="crosshook-game-details-modal__cover-wrap" aria-label="Cover preview">
        {coverLoading ? (
          <div className="crosshook-game-details-modal__cover crosshook-game-details-modal__cover--skeleton crosshook-skeleton" />
        ) : coverArtUrl ? (
          <img className="crosshook-game-details-modal__cover" src={coverArtUrl} alt={`${displayName} cover art`} />
        ) : (
          <div className="crosshook-game-details-modal__cover-fallback" aria-hidden="true">
            {displayName.slice(0, 2).toUpperCase()}
          </div>
        )}
      </div>
    </section>
  );
}
