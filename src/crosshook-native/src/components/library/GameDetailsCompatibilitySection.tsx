import { useProtonDbLookup } from '../../hooks/useProtonDbLookup';

export interface GameDetailsCompatibilitySectionProps {
  steamAppId: string;
}

export function GameDetailsCompatibilitySection({ steamAppId }: GameDetailsCompatibilitySectionProps) {
  const trimmed = steamAppId.trim();
  const hasAppId = /^\d+$/.test(trimmed);
  const proton = useProtonDbLookup(hasAppId ? trimmed : '');
  let statusMessage: JSX.Element | null = null;

  if (!hasAppId) {
    statusMessage = <p className="crosshook-hero-detail__muted">ProtonDB data needs a numeric Steam App ID.</p>;
  } else if (proton.loading || proton.state === 'loading') {
    statusMessage = <p className="crosshook-hero-detail__muted">Loading ProtonDB summary…</p>;
  } else if (proton.isUnavailable) {
    statusMessage = (
      <p className="crosshook-hero-detail__muted">ProtonDB data is unavailable (offline or not cached).</p>
    );
  } else if (proton.isStale) {
    statusMessage = <p className="crosshook-hero-detail__stale">Showing cached ProtonDB data; it may be stale.</p>;
  }

  return (
    <section
      className="crosshook-hero-detail__section crosshook-hero-detail__section--card"
      aria-labelledby="crosshook-hero-detail-proton-heading"
    >
      <h3 id="crosshook-hero-detail-proton-heading" className="crosshook-hero-detail__section-title">
        ProtonDB compatibility
      </h3>
      {statusMessage}
      {hasAppId && proton.snapshot && (proton.state === 'ready' || proton.state === 'stale') ? (
        <div className="crosshook-hero-detail__proton-summary">
          <p className="crosshook-hero-detail__text">
            <span className="crosshook-hero-detail__label">Tier: </span>
            {String(proton.snapshot.tier)}
          </p>
          {proton.snapshot.confidence ? (
            <p className="crosshook-hero-detail__text">
              <span className="crosshook-hero-detail__label">Confidence: </span>
              {proton.snapshot.confidence}
            </p>
          ) : null}
          {proton.snapshot.total_reports != null ? (
            <p className="crosshook-hero-detail__text">
              <span className="crosshook-hero-detail__label">Reports: </span>
              {proton.snapshot.total_reports}
            </p>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}
