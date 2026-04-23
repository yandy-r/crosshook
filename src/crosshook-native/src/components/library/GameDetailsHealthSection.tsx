import { useId } from 'react';
import type { OfflineReadinessReport } from '../../types';
import type { EnrichedProfileHealthReport } from '../../types/health';

export interface GameDetailsHealthSectionProps {
  profileName: string;
  healthReport: EnrichedProfileHealthReport | undefined;
  healthLoading: boolean;
  offlineReport: OfflineReadinessReport | undefined;
  offlineError: string | null;
}

function readinessLabel(score: number, state: string): string {
  return `${state.replace(/_/g, ' ')} (${score}%)`;
}

export function GameDetailsHealthSection({
  profileName,
  healthReport,
  healthLoading,
  offlineReport,
  offlineError,
}: GameDetailsHealthSectionProps) {
  const healthHeadingId = useId();
  return (
    <section
      className="crosshook-hero-detail__section crosshook-hero-detail__section--card"
      aria-labelledby={healthHeadingId}
    >
      <h3 id={healthHeadingId} className="crosshook-hero-detail__section-title">
        Health and offline readiness
      </h3>
      <div className="crosshook-hero-detail__subsection">
        <h4 className="crosshook-hero-detail__subsection-title">Profile health</h4>
        {healthLoading && !healthReport ? (
          <p className="crosshook-hero-detail__muted">Loading health snapshot…</p>
        ) : null}
        {!healthLoading && !healthReport ? (
          <p className="crosshook-hero-detail__muted">
            No health data for <span className="crosshook-hero-detail__mono">{profileName}</span> yet.
          </p>
        ) : null}
        {healthReport ? (
          <div className="crosshook-hero-detail__health-block">
            <div className="crosshook-hero-detail__kv-list">
              <p className="crosshook-hero-detail__kv-item">
                <span className="crosshook-hero-detail__kv-key">Status</span>
                <span className="crosshook-hero-detail__kv-value">{healthReport.status}</span>
              </p>
              <p className="crosshook-hero-detail__kv-item">
                <span className="crosshook-hero-detail__kv-key">Launch method</span>
                <span className="crosshook-hero-detail__kv-value">{healthReport.launch_method}</span>
              </p>
              <p className="crosshook-hero-detail__kv-item">
                <span className="crosshook-hero-detail__kv-key">Checked</span>
                <span className="crosshook-hero-detail__kv-value crosshook-hero-detail__text--small">
                  {healthReport.checked_at}
                </span>
              </p>
            </div>
            {healthReport.issues.length > 0 ? (
              <ul className="crosshook-hero-detail__issue-list">
                {healthReport.issues.slice(0, 5).map((issue) => (
                  <li key={`${issue.path}-${issue.message}`} className="crosshook-hero-detail__issue">
                    <span className="crosshook-hero-detail__issue-severity">{issue.severity}</span>
                    {issue.message}
                  </li>
                ))}
              </ul>
            ) : null}
          </div>
        ) : null}
      </div>
      <div className="crosshook-hero-detail__subsection">
        <h4 className="crosshook-hero-detail__subsection-title">Offline readiness</h4>
        {offlineError ? <p className="crosshook-hero-detail__warn">{offlineError}</p> : null}
        {!offlineReport && !offlineError ? (
          <p className="crosshook-hero-detail__muted">Offline readiness has not been computed for this profile.</p>
        ) : null}
        {offlineReport ? (
          <div className="crosshook-hero-detail__health-block">
            <div className="crosshook-hero-detail__kv-list">
              <p className="crosshook-hero-detail__kv-item">
                <span className="crosshook-hero-detail__kv-key">Readiness</span>
                <span className="crosshook-hero-detail__kv-value">
                  {readinessLabel(offlineReport.score, offlineReport.readiness_state)}
                </span>
              </p>
              <p className="crosshook-hero-detail__kv-item">
                <span className="crosshook-hero-detail__kv-key">Checked</span>
                <span className="crosshook-hero-detail__kv-value crosshook-hero-detail__text--small">
                  {offlineReport.checked_at}
                </span>
              </p>
            </div>
            {offlineReport.blocking_reasons.length > 0 ? (
              <ul className="crosshook-hero-detail__issue-list">
                {offlineReport.blocking_reasons.map((reason) => (
                  <li key={reason} className="crosshook-hero-detail__issue">
                    {reason}
                  </li>
                ))}
              </ul>
            ) : null}
          </div>
        ) : null}
      </div>
    </section>
  );
}
