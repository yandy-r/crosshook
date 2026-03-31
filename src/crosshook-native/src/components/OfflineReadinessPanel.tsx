import type { HealthIssue } from '../types/health';
import type { OfflineReadinessReport } from '../types/offline';

function healthSeverityLabel(severity: HealthIssue['severity']): string {
  switch (severity) {
    case 'error':
      return 'Error';
    case 'warning':
      return 'Warning';
    case 'info':
    default:
      return 'Info';
  }
}

function checkIcon(severity: HealthIssue['severity']): string {
  switch (severity) {
    case 'error':
      return '\u2715';
    case 'warning':
      return '!';
    case 'info':
    default:
      return '\u2713';
  }
}

export type OfflineReadinessPanelProps = {
  report: OfflineReadinessReport | null;
  error?: string | null;
  loading?: boolean;
};

export function OfflineReadinessPanel({ report, error, loading }: OfflineReadinessPanelProps) {
  return (
    <div className="crosshook-offline-readiness-panel" data-crosshook-focus-root="true">
      {error ? (
        <p className="crosshook-launch-panel__feedback-help" role="status">
          {error}
        </p>
      ) : null}
      {loading && !report ? (
        <p className="crosshook-launch-panel__feedback-help" role="status">Loading offline readiness…</p>
      ) : null}
      {report && report.blocking_reasons.length > 0 ? (
        <div className="crosshook-offline-readiness-panel__blocking">
          <p className="crosshook-launch-panel__feedback-title">Blocking reasons</p>
          <ul className="crosshook-launch-panel__feedback-list">
            {report.blocking_reasons.map((reason) => (
              <li key={reason} className="crosshook-launch-panel__feedback-item">
                <p className="crosshook-launch-panel__feedback-help">{reason}</p>
              </li>
            ))}
          </ul>
        </div>
      ) : null}
      {report && report.checks.length > 0 ? (
        <ul className="crosshook-launch-panel__feedback-list" aria-label="Offline readiness checks">
          {report.checks.map((check, index) => (
            <li
              key={`${check.field}-${check.message}-${index}`}
              className="crosshook-launch-panel__feedback-item crosshook-offline-readiness-panel__row"
            >
              <span className="crosshook-offline-readiness-panel__icon" aria-hidden="true">
                {checkIcon(check.severity)}
              </span>
              <div>
                <div className="crosshook-launch-panel__feedback-header">
                  <span
                    className="crosshook-launch-panel__feedback-badge"
                    data-severity={check.severity}
                  >
                    {healthSeverityLabel(check.severity)}
                  </span>
                  <p className="crosshook-launch-panel__feedback-title">
                    {check.field}: {check.message}
                  </p>
                </div>
                {check.path ? <p className="crosshook-launch-panel__feedback-help">{check.path}</p> : null}
                {check.remediation ? (
                  <p className="crosshook-launch-panel__feedback-help">{check.remediation}</p>
                ) : null}
              </div>
            </li>
          ))}
        </ul>
      ) : null}
      {report && !loading ? (
        <p className="crosshook-preview-modal__timestamp">Checked {report.checked_at}</p>
      ) : null}
    </div>
  );
}
