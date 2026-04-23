import { useId } from 'react';
import { useProtonDbLookup } from '../../hooks/useProtonDbLookup';

export interface GameDetailsCompatibilitySectionProps {
  steamAppId: string;
}

export function GameDetailsCompatibilitySection({ steamAppId }: GameDetailsCompatibilitySectionProps) {
  const protonHeadingId = useId();
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
      aria-labelledby={protonHeadingId}
    >
      <h3 id={protonHeadingId} className="crosshook-hero-detail__section-title">
        ProtonDB compatibility
      </h3>
      {statusMessage}
      {hasAppId && proton.snapshot && (proton.state === 'ready' || proton.state === 'stale') ? (
        <div className="crosshook-hero-detail__subsection crosshook-hero-detail__proton-summary">
          <h4 className="crosshook-hero-detail__subsection-title">Snapshot summary</h4>
          <div className="crosshook-hero-detail__kv-list">
            <p className="crosshook-hero-detail__kv-item">
              <span className="crosshook-hero-detail__kv-key">Tier</span>
              <span className="crosshook-hero-detail__kv-value">{String(proton.snapshot.tier)}</span>
            </p>
            {proton.snapshot.confidence ? (
              <p className="crosshook-hero-detail__kv-item">
                <span className="crosshook-hero-detail__kv-key">Confidence</span>
                <span className="crosshook-hero-detail__kv-value">{proton.snapshot.confidence}</span>
              </p>
            ) : null}
            {proton.snapshot.total_reports != null ? (
              <p className="crosshook-hero-detail__kv-item">
                <span className="crosshook-hero-detail__kv-key">Reports</span>
                <span className="crosshook-hero-detail__kv-value">{proton.snapshot.total_reports}</span>
              </p>
            ) : null}
          </div>
        </div>
      ) : null}
    </section>
  );
}
