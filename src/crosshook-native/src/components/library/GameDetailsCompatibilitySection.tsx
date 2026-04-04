import { useProtonDbLookup } from '../../hooks/useProtonDbLookup';

export interface GameDetailsCompatibilitySectionProps {
  steamAppId: string;
}

export function GameDetailsCompatibilitySection({ steamAppId }: GameDetailsCompatibilitySectionProps) {
  const trimmed = steamAppId.trim();
  const hasAppId = /^\d+$/.test(trimmed);
  const proton = useProtonDbLookup(hasAppId ? trimmed : '');

  return (
    <section className="crosshook-game-details-modal__section" aria-labelledby="crosshook-game-details-proton-heading">
      <h3 id="crosshook-game-details-proton-heading" className="crosshook-game-details-modal__section-title">
        ProtonDB compatibility
      </h3>
      {!hasAppId ? (
        <p className="crosshook-game-details-modal__muted">ProtonDB data needs a numeric Steam App ID.</p>
      ) : null}
      {hasAppId && (proton.loading || proton.state === 'loading') ? (
        <p className="crosshook-game-details-modal__muted">Loading ProtonDB summary…</p>
      ) : null}
      {hasAppId && proton.isUnavailable && !proton.loading ? (
        <p className="crosshook-game-details-modal__muted">ProtonDB data is unavailable (offline or not cached).</p>
      ) : null}
      {hasAppId && proton.isStale ? (
        <p className="crosshook-game-details-modal__stale">Showing cached ProtonDB data; it may be stale.</p>
      ) : null}
      {hasAppId && proton.snapshot && (proton.state === 'ready' || proton.state === 'stale') ? (
        <div className="crosshook-game-details-modal__proton-summary">
          <p className="crosshook-game-details-modal__text">
            <span className="crosshook-game-details-modal__label">Tier: </span>
            {String(proton.snapshot.tier)}
          </p>
          {proton.snapshot.confidence ? (
            <p className="crosshook-game-details-modal__text">
              <span className="crosshook-game-details-modal__label">Confidence: </span>
              {proton.snapshot.confidence}
            </p>
          ) : null}
          {proton.snapshot.total_reports != null ? (
            <p className="crosshook-game-details-modal__text">
              <span className="crosshook-game-details-modal__label">Reports: </span>
              {proton.snapshot.total_reports}
            </p>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}
