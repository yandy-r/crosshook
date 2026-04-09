import type { EnrichedProfileHealthReport } from '../../types/health';
import type { OfflineReadinessReport } from '../../types';

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
  return (
    <section
      className="crosshook-game-details-modal__section crosshook-game-details-modal__section--card"
      aria-labelledby="crosshook-game-details-health-heading"
    >
      <h3 id="crosshook-game-details-health-heading" className="crosshook-game-details-modal__section-title">
        Health and offline readiness
      </h3>
      <div className="crosshook-game-details-modal__subsection">
        <h4 className="crosshook-game-details-modal__subsection-title">Profile health</h4>
        {healthLoading && !healthReport ? (
          <p className="crosshook-game-details-modal__muted">Loading health snapshot…</p>
        ) : null}
        {!healthLoading && !healthReport ? (
          <p className="crosshook-game-details-modal__muted">
            No health data for <span className="crosshook-game-details-modal__mono">{profileName}</span> yet.
          </p>
        ) : null}
        {healthReport ? (
          <div className="crosshook-game-details-modal__health-block">
            <p className="crosshook-game-details-modal__text">
              <span className="crosshook-game-details-modal__label">Status: </span>
              {healthReport.status}
            </p>
            <p className="crosshook-game-details-modal__text">
              <span className="crosshook-game-details-modal__label">Launch method: </span>
              {healthReport.launch_method}
            </p>
            <p className="crosshook-game-details-modal__text crosshook-game-details-modal__text--small">
              Checked {healthReport.checked_at}
            </p>
            {healthReport.issues.length > 0 ? (
              <ul className="crosshook-game-details-modal__issue-list">
                {healthReport.issues.slice(0, 5).map((issue) => (
                  <li key={`${issue.path}-${issue.message}`} className="crosshook-game-details-modal__issue">
                    <span className="crosshook-game-details-modal__issue-severity">{issue.severity}</span>
                    {issue.message}
                  </li>
                ))}
              </ul>
            ) : null}
          </div>
        ) : null}
      </div>
      <div className="crosshook-game-details-modal__subsection">
        <h4 className="crosshook-game-details-modal__subsection-title">Offline readiness</h4>
        {offlineError ? <p className="crosshook-game-details-modal__warn">{offlineError}</p> : null}
        {!offlineReport && !offlineError ? (
          <p className="crosshook-game-details-modal__muted">
            Offline readiness has not been computed for this profile.
          </p>
        ) : null}
        {offlineReport ? (
          <div className="crosshook-game-details-modal__health-block">
            <p className="crosshook-game-details-modal__text">
              <span className="crosshook-game-details-modal__label">Readiness: </span>
              {readinessLabel(offlineReport.score, offlineReport.readiness_state)}
            </p>
            {offlineReport.blocking_reasons.length > 0 ? (
              <ul className="crosshook-game-details-modal__issue-list">
                {offlineReport.blocking_reasons.map((reason) => (
                  <li key={reason} className="crosshook-game-details-modal__issue">
                    {reason}
                  </li>
                ))}
              </ul>
            ) : null}
            <p className="crosshook-game-details-modal__text crosshook-game-details-modal__text--small">
              Checked {offlineReport.checked_at}
            </p>
          </div>
        ) : null}
      </div>
    </section>
  );
}
