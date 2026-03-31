import type { OfflineReadinessReport } from '../types/offline';

export type OfflineStatusBadgeProps = {
  report?: OfflineReadinessReport | null;
  loading?: boolean;
  /** When no full report, pass score/readiness for dashboard cached rows. */
  score?: number | null;
  readinessState?: string | null;
  /** Shorter labels for dense tables (e.g. health dashboard). */
  compact?: boolean;
};

function badgeStyle(
  score: number | null | undefined,
  readinessState: string | null | undefined,
  loading: boolean,
  compact: boolean,
): { label: string; classSuffix: string; aria: string } {
  if (loading) {
    return {
      label: compact ? '…' : 'Computing…',
      classSuffix: 'computing',
      aria: 'Offline readiness: computing',
    };
  }
  const s = score ?? null;
  const st = readinessState ?? '';
  if (s === null || s === undefined || st === 'unconfigured' || Number.isNaN(s)) {
    return {
      label: compact ? '?' : 'Unknown',
      classSuffix: 'unknown',
      aria: `Offline readiness: unknown${st ? `, state ${st}` : ''}`,
    };
  }
  if (s >= 80) {
    return {
      label: compact ? 'Ready' : 'Offline Ready',
      classSuffix: 'ready',
      aria: `Offline readiness: ready, score ${s} of 100`,
    };
  }
  if (s >= 50) {
    return {
      label: 'Partial',
      classSuffix: 'partial',
      aria: `Offline readiness: partial, score ${s} of 100`,
    };
  }
  return {
    label: compact ? 'Low' : 'Not Ready',
    classSuffix: 'not-ready',
    aria: `Offline readiness: not ready, score ${s} of 100`,
  };
}

export function OfflineStatusBadge({
  report,
  loading = false,
  score: scoreProp,
  readinessState: rsProp,
  compact = false,
}: OfflineStatusBadgeProps) {
  const score = report?.score ?? scoreProp ?? null;
  const readinessState = report?.readiness_state ?? rsProp ?? null;
  const { label, classSuffix, aria } = badgeStyle(score, readinessState, loading, compact);

  const border =
    classSuffix === 'ready'
      ? 'var(--offline-ready)'
      : classSuffix === 'partial'
        ? 'var(--offline-partial)'
        : classSuffix === 'not-ready'
          ? 'var(--offline-not-ready)'
          : classSuffix === 'unknown'
            ? 'var(--offline-unknown)'
            : 'var(--crosshook-color-border)';

  const background =
    classSuffix === 'ready'
      ? 'color-mix(in srgb, var(--offline-ready) 22%, transparent)'
      : classSuffix === 'partial'
        ? 'color-mix(in srgb, var(--offline-partial) 22%, transparent)'
        : classSuffix === 'not-ready'
          ? 'color-mix(in srgb, var(--offline-not-ready) 22%, transparent)'
          : classSuffix === 'unknown'
            ? 'color-mix(in srgb, var(--offline-unknown) 18%, transparent)'
            : 'var(--crosshook-color-surface-strong)';

  return (
    <span
      className={`crosshook-status-chip crosshook-offline-badge crosshook-offline-badge--${classSuffix}${compact ? ' crosshook-offline-badge--compact' : ''}`}
      data-crosshook-focus-root="true"
      aria-label={aria}
      style={{
        border: `1px solid ${border}`,
        background,
        display: 'inline-flex',
        alignItems: 'center',
        gap: 6,
      }}
    >
      {loading ? (
        <span className="crosshook-offline-badge__spinner" aria-hidden="true">
          ◌
        </span>
      ) : null}
      <span>{label}</span>
      {!loading && score !== null && score !== undefined && !Number.isNaN(score) ? (
        <span className="crosshook-muted" style={{ fontWeight: 600 }}>
          {score}
        </span>
      ) : null}
    </span>
  );
}
