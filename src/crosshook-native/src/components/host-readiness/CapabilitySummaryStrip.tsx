type CapabilitySummaryTone = 'available' | 'degraded' | 'unavailable';

export interface CapabilitySummaryStripProps {
  requiredToolsReady: number;
  requiredToolsTotal: number;
  optionalCapabilitiesAvailable: number;
  optionalCapabilitiesTotal: number;
  showStaleBadge?: boolean;
  staleBadgeLabel?: string;
  className?: string;
}

function joinClasses(...values: Array<string | false | null | undefined>): string {
  return values.filter((value): value is string => Boolean(value && value.trim().length > 0)).join(' ');
}

function assertNonNegativeInteger(value: number, label: string): void {
  if (!Number.isInteger(value) || value < 0) {
    throw new Error(`${label} must be a non-negative integer.`);
  }
}

function resolveCountTone(readyCount: number, totalCount: number): CapabilitySummaryTone {
  if (totalCount === 0 || readyCount >= totalCount) {
    return 'available';
  }

  if (readyCount === 0) {
    return 'unavailable';
  }

  return 'degraded';
}

function formatCountLabel(count: number, singular: string, plural: string): string {
  return `${count} ${count === 1 ? singular : plural}`;
}

function formatCapabilitySummary(
  readyCount: number,
  totalCount: number,
  singular: string,
  plural: string,
  suffix: string
): string {
  if (totalCount === 0) {
    return `0 ${plural} ${suffix}`;
  }

  return `${formatCountLabel(readyCount, singular, plural)} ${suffix} of ${totalCount}`;
}

export function CapabilitySummaryStrip({
  requiredToolsReady,
  requiredToolsTotal,
  optionalCapabilitiesAvailable,
  optionalCapabilitiesTotal,
  showStaleBadge = false,
  staleBadgeLabel = 'Stale data',
  className,
}: CapabilitySummaryStripProps) {
  assertNonNegativeInteger(requiredToolsReady, 'requiredToolsReady');
  assertNonNegativeInteger(requiredToolsTotal, 'requiredToolsTotal');
  assertNonNegativeInteger(optionalCapabilitiesAvailable, 'optionalCapabilitiesAvailable');
  assertNonNegativeInteger(optionalCapabilitiesTotal, 'optionalCapabilitiesTotal');

  if (requiredToolsReady > requiredToolsTotal) {
    throw new Error('requiredToolsReady cannot exceed requiredToolsTotal.');
  }

  if (optionalCapabilitiesAvailable > optionalCapabilitiesTotal) {
    throw new Error('optionalCapabilitiesAvailable cannot exceed optionalCapabilitiesTotal.');
  }

  const requiredToolsTone = resolveCountTone(requiredToolsReady, requiredToolsTotal);
  const optionalCapabilitiesTone = resolveCountTone(optionalCapabilitiesAvailable, optionalCapabilitiesTotal);

  return (
    <div
      className={className}
      style={{
        display: 'flex',
        flexWrap: 'wrap',
        alignItems: 'center',
        gap: '10px',
      }}
    >
      <span
        className={joinClasses(
          'crosshook-status-chip',
          'crosshook-host-tool-dashboard__status-chip',
          `crosshook-host-tool-dashboard__status-chip--${requiredToolsTone}`
        )}
        title={`${requiredToolsReady} of ${requiredToolsTotal} required host tools are currently ready.`}
      >
        {formatCapabilitySummary(requiredToolsReady, requiredToolsTotal, 'required tool', 'required tools', 'ready')}
      </span>
      <span
        className={joinClasses(
          'crosshook-status-chip',
          'crosshook-host-tool-dashboard__status-chip',
          `crosshook-host-tool-dashboard__status-chip--${optionalCapabilitiesTone}`
        )}
        title={`${optionalCapabilitiesAvailable} of ${optionalCapabilitiesTotal} optional capabilities are currently available.`}
      >
        {formatCapabilitySummary(
          optionalCapabilitiesAvailable,
          optionalCapabilitiesTotal,
          'optional capability',
          'optional capabilities',
          'available'
        )}
      </span>
      {showStaleBadge ? (
        <span
          className={joinClasses(
            'crosshook-status-chip',
            'crosshook-host-tool-dashboard__status-chip',
            'crosshook-host-tool-dashboard__status-chip--degraded'
          )}
          title="These capability counts are based on a stale cached readiness snapshot."
        >
          {staleBadgeLabel}
        </span>
      ) : null}
    </div>
  );
}

export default CapabilitySummaryStrip;
