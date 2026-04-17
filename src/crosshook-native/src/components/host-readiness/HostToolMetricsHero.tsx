import type { Capability, HostToolCheckResult } from '../../types/onboarding';

export interface HostToolMetricsHeroProps {
  toolChecks: HostToolCheckResult[];
  capabilities: Capability[];
  loading: boolean;
}

interface MetricCardProps {
  count: number | null;
  label: string;
  accentColor: string;
  loading: boolean;
}

function MetricCard({ count, label, accentColor, loading }: MetricCardProps) {
  const display = loading || count === null ? '—' : String(count);
  return (
    <div
      className="crosshook-card crosshook-host-tool-dashboard-card"
      style={{ borderLeftColor: accentColor }}
      aria-busy={loading}
    >
      <div className="crosshook-host-tool-dashboard-card__count" style={{ color: loading ? undefined : accentColor }}>
        {display}
      </div>
      <div className="crosshook-host-tool-dashboard-card__label crosshook-muted">{label}</div>
    </div>
  );
}

function SkeletonCard() {
  return (
    <div className="crosshook-card crosshook-host-tool-dashboard-card" aria-hidden="true">
      <div className="crosshook-host-tool-dashboard-skeleton crosshook-host-tool-dashboard-skeleton--count" />
      <div className="crosshook-host-tool-dashboard-skeleton crosshook-host-tool-dashboard-skeleton--label" />
    </div>
  );
}

function SkeletonHero() {
  return (
    <>
      <SkeletonCard />
      <SkeletonCard />
      <SkeletonCard />
      <SkeletonCard />
    </>
  );
}

export function HostToolMetricsHero({ toolChecks, capabilities, loading }: HostToolMetricsHeroProps) {
  const totalTools = toolChecks.length;
  const requiredReady = toolChecks.filter((t) => t.is_required && t.is_available).length;
  const requiredTotal = toolChecks.filter((t) => t.is_required).length;
  const missingRequired = requiredTotal - requiredReady;
  const optionalAvailable = capabilities.filter((c) => c.state === 'available').length;
  const optionalTotal = capabilities.length;

  const requiredAccent =
    missingRequired === 0 && requiredTotal > 0
      ? 'var(--crosshook-color-success)'
      : missingRequired > 0
        ? 'var(--crosshook-color-danger)'
        : 'var(--crosshook-color-text-subtle)';

  const optionalAccent =
    optionalTotal === 0
      ? 'var(--crosshook-color-text-subtle)'
      : optionalAvailable === optionalTotal
        ? 'var(--crosshook-color-success)'
        : optionalAvailable === 0
          ? 'var(--crosshook-color-danger)'
          : 'var(--crosshook-color-warning)';

  const missingAccent = missingRequired > 0 ? 'var(--crosshook-color-danger)' : 'var(--crosshook-color-text-subtle)';

  const showSkeleton = loading && totalTools === 0;

  return (
    <section
      className="crosshook-host-tool-dashboard-cards"
      aria-busy={loading}
      aria-label="Host tool readiness summary"
    >
      {showSkeleton ? (
        <SkeletonHero />
      ) : (
        <>
          <MetricCard
            count={totalTools}
            label="Total tools"
            accentColor="var(--crosshook-color-accent)"
            loading={loading}
          />
          <MetricCard
            count={requiredReady}
            label={requiredTotal > 0 ? `Required ready (of ${requiredTotal})` : 'Required ready'}
            accentColor={requiredAccent}
            loading={loading}
          />
          <MetricCard
            count={optionalAvailable}
            label={optionalTotal > 0 ? `Capabilities available (of ${optionalTotal})` : 'Capabilities available'}
            accentColor={optionalAccent}
            loading={loading}
          />
          <MetricCard count={missingRequired} label="Missing required" accentColor={missingAccent} loading={loading} />
        </>
      )}
    </section>
  );
}

export default HostToolMetricsHero;
