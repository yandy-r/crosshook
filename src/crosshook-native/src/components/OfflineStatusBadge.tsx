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
  if (s === null || Number.isNaN(s) || st === 'unconfigured') {
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

  const BORDER_COLORS: Record<string, string> = {
    ready: 'var(--crosshook-offline-ready)',
    partial: 'var(--crosshook-offline-partial)',
    'not-ready': 'var(--crosshook-offline-not-ready)',
    unknown: 'var(--crosshook-offline-unknown)',
  };

  const BG_COLORS: Record<string, string> = {
    ready: 'color-mix(in srgb, var(--crosshook-offline-ready) 22%, transparent)',
    partial: 'color-mix(in srgb, var(--crosshook-offline-partial) 22%, transparent)',
    'not-ready': 'color-mix(in srgb, var(--crosshook-offline-not-ready) 22%, transparent)',
    unknown: 'color-mix(in srgb, var(--crosshook-offline-unknown) 18%, transparent)',
  };

  const border = BORDER_COLORS[classSuffix] ?? 'var(--crosshook-color-border)';
  const background = BG_COLORS[classSuffix] ?? 'var(--crosshook-color-surface-strong)';

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
