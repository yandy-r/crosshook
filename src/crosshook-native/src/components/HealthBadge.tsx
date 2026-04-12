import type { TrendDirection } from '../hooks/useProfileHealth';
import type { HealthStatus, ProfileHealthMetadata, ProfileHealthReport } from '../types/health';

type HealthBadgeProps = {
  metadata?: ProfileHealthMetadata | null;
  trend?: TrendDirection | null;
  tooltip?: string | null;
  onClick?: () => void;
} & ({ status: HealthStatus; report?: never } | { status?: never; report: ProfileHealthReport });

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

export function HealthBadge({
  status,
  report,
  metadata = null,
  trend = null,
  tooltip = null,
  onClick,
}: HealthBadgeProps) {
  const resolvedStatus: HealthStatus = status ?? report.status;
  const rating = STATUS_TO_RATING[resolvedStatus];

  const showFailureTrend = metadata !== null && metadata.failure_count_30d >= 2;
  const failureCount = metadata?.failure_count_30d ?? 0;
  const trendColor = failureCount >= 5 ? 'var(--crosshook-color-warning)' : 'var(--crosshook-color-text-muted)';

  const showVersionMismatch =
    metadata !== null &&
    (metadata.version_status === 'game_updated' ||
      metadata.version_status === 'trainer_changed' ||
      metadata.version_status === 'both_changed');
  const versionMismatchLabel =
    metadata?.version_status === 'both_changed'
      ? 'game and trainer version changed'
      : metadata?.version_status === 'trainer_changed'
        ? 'trainer version changed'
        : 'game version changed';

  const ariaLabel = showFailureTrend
    ? `Health status: ${STATUS_LABEL[resolvedStatus]}, ${failureCount} failures in last 30 days`
    : `Health status: ${STATUS_LABEL[resolvedStatus]}`;

  const isInteractive = typeof onClick === 'function';

  return (
    <span
      style={{
        position: 'relative',
        display: 'inline-flex',
        alignItems: 'center',
        gap: '4px',
        cursor: isInteractive ? 'pointer' : undefined,
      }}
      title={tooltip ?? undefined}
      onClick={
        isInteractive
          ? (e) => {
              e.preventDefault();
              e.stopPropagation();
              onClick?.();
            }
          : undefined
      }
      role={isInteractive ? 'button' : undefined}
      tabIndex={isInteractive ? 0 : undefined}
      onKeyDown={
        isInteractive
          ? (e) => {
              if (e.key === 'Enter' || e.key === ' ') {
                e.preventDefault();
                e.stopPropagation();
                onClick?.();
              }
            }
          : undefined
      }
    >
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
          {'\u2191'}
          {failureCount}x
        </span>
      )}
      {(trend === 'got_worse' || trend === 'got_better') && (
        <span
          role="img"
          aria-label={trend === 'got_worse' ? 'trending worse' : 'trending better'}
          style={{
            fontSize: '0.75em',
            fontWeight: 700,
            color: trend === 'got_worse' ? 'var(--crosshook-color-warning)' : 'var(--crosshook-color-success)',
            lineHeight: 1,
            whiteSpace: 'nowrap',
          }}
        >
          {trend === 'got_worse' ? '\u2193' : '\u2191'}
        </span>
      )}
      {showVersionMismatch && (
        <span
          role="img"
          aria-label={versionMismatchLabel}
          style={{
            fontSize: '0.7em',
            fontWeight: 600,
            color: 'var(--crosshook-color-warning)',
            lineHeight: 1,
            whiteSpace: 'nowrap',
          }}
        >
          {'\u26a0'}
        </span>
      )}
    </span>
  );
}
