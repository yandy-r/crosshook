import type { HealthStatus, ProfileHealthReport } from '../types/health';

type HealthBadgeProps =
  | { status: HealthStatus; report?: never }
  | { status?: never; report: ProfileHealthReport };

const STATUS_TO_RATING: Record<HealthStatus, string> = {
  healthy: 'working',
  stale: 'partial',
  broken: 'broken',
};

const STATUS_ICON: Record<HealthStatus, string> = {
  healthy: '\u2713',
  stale: '\u26a0',
  broken: '\u2715',
};

const STATUS_LABEL: Record<HealthStatus, string> = {
  healthy: 'Healthy',
  stale: 'Stale',
  broken: 'Broken',
};

export function HealthBadge({ status, report }: HealthBadgeProps) {
  const resolvedStatus: HealthStatus = status ?? report.status;
  const rating = STATUS_TO_RATING[resolvedStatus];

  return (
    <span
      className={`crosshook-status-chip crosshook-compatibility-badge crosshook-compatibility-badge--${rating}`}
      aria-label={`Health status: ${STATUS_LABEL[resolvedStatus]}`}
    >
      <span aria-hidden="true">{STATUS_ICON[resolvedStatus]}</span>
      {STATUS_LABEL[resolvedStatus]}
    </span>
  );
}
