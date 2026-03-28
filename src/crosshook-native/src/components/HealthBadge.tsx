import type { HealthStatus, ProfileHealthReport, ProfileHealthMetadata } from '../types/health';

type HealthBadgeProps =
  | { status: HealthStatus; report?: never; metadata?: ProfileHealthMetadata | null }
  | { status?: never; report: ProfileHealthReport; metadata?: ProfileHealthMetadata | null };

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

export function HealthBadge({ status, report, metadata = null }: HealthBadgeProps) {
  const resolvedStatus: HealthStatus = status ?? report.status;
  const rating = STATUS_TO_RATING[resolvedStatus];

  const showFailureTrend = metadata !== null && metadata.failure_count_30d >= 2;
  const failureCount = metadata?.failure_count_30d ?? 0;
  const trendColor = failureCount >= 5
    ? 'var(--crosshook-color-warning)'
    : 'var(--crosshook-color-text-muted)';

  const ariaLabel = showFailureTrend
    ? `Health status: ${STATUS_LABEL[resolvedStatus]}, ${failureCount} failures in last 30 days`
    : `Health status: ${STATUS_LABEL[resolvedStatus]}`;

  return (
    <span style={{ position: 'relative', display: 'inline-flex', alignItems: 'center', gap: '4px' }}>
      <span
        className={`crosshook-status-chip crosshook-compatibility-badge crosshook-compatibility-badge--${rating}`}
        aria-label={ariaLabel}
      >
        <span aria-hidden="true">{STATUS_ICON[resolvedStatus]}</span>
        {STATUS_LABEL[resolvedStatus]}
      </span>
      {showFailureTrend && (
        <span
          aria-hidden="true"
          style={{
            fontSize: '0.7em',
            fontWeight: 600,
            color: trendColor,
            lineHeight: 1,
            whiteSpace: 'nowrap',
          }}
        >
          {'\u2191'}{failureCount}x
        </span>
      )}
    </span>
  );
}
