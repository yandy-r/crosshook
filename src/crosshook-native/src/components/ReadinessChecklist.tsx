import type { HealthIssue, HealthIssueSeverity } from '../types/health';

interface ReadinessChecklistProps {
  checks: HealthIssue[];
  isLoading: boolean;
}

function getSeverityVariant(severity: HealthIssueSeverity): string {
  switch (severity) {
    case 'info':
      return 'found';
    case 'warning':
      return 'ambiguous';
    case 'error':
      return 'not-found';
  }
}

function getSeverityIcon(severity: HealthIssueSeverity): string {
  switch (severity) {
    case 'info':
      return '✓';
    case 'warning':
      return '⚠';
    case 'error':
      return '✕';
  }
}

function getSeverityLabel(severity: HealthIssueSeverity): string {
  switch (severity) {
    case 'info':
      return 'OK';
    case 'warning':
      return 'Warning';
    case 'error':
      return 'Error';
  }
}

interface CheckCardProps {
  check: HealthIssue;
}

function CheckCard({ check }: CheckCardProps) {
  const variant = getSeverityVariant(check.severity);
  const icon = getSeverityIcon(check.severity);
  const label = getSeverityLabel(check.severity);

  return (
    <div className={`crosshook-auto-populate__field-card crosshook-auto-populate__field-card--${variant} crosshook-readiness-checklist__card`}>
      <div className="crosshook-readiness-checklist__card-header">
        <span
          className={`crosshook-readiness-checklist__icon crosshook-auto-populate__field-state--${variant}`}
          aria-hidden="true"
        >
          {icon}
        </span>
        <span className="crosshook-readiness-checklist__message">{check.message}</span>
        <span className={`crosshook-readiness-checklist__badge crosshook-auto-populate__field-state--${variant}`}>
          {label}
        </span>
      </div>
      {check.remediation ? (
        <div className="crosshook-readiness-checklist__remediation">{check.remediation}</div>
      ) : null}
    </div>
  );
}

export function ReadinessChecklist({ checks, isLoading }: ReadinessChecklistProps) {
  if (isLoading) {
    return (
      <section className="crosshook-readiness-checklist" aria-label="System readiness checks" aria-busy="true">
        <div className="crosshook-readiness-checklist__loading">
          <div className="crosshook-readiness-checklist__spinner" aria-hidden="true" />
          <span>Running readiness checks...</span>
        </div>
      </section>
    );
  }

  if (checks.length === 0) {
    return (
      <section className="crosshook-readiness-checklist" aria-label="System readiness checks">
        <div className="crosshook-readiness-checklist__empty">No checks have been run yet.</div>
      </section>
    );
  }

  return (
    <section className="crosshook-readiness-checklist" aria-label="System readiness checks">
      <ul className="crosshook-readiness-checklist__list" role="list">
        {checks.map((check, index) => (
          <li key={`${check.field}-${index}`}>
            <CheckCard check={check} />
          </li>
        ))}
      </ul>
    </section>
  );
}

export default ReadinessChecklist;
