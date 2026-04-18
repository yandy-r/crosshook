import type { CardTrend } from './constants';

function TrendArrow({ trend, improving }: { trend: CardTrend; improving: boolean }) {
  if (trend === null) return null;
  const isPositive = (trend === 'up' && improving) || (trend === 'down' && !improving);
  const color = isPositive ? 'var(--crosshook-color-success)' : 'var(--crosshook-color-danger)';
  const label = trend === 'up' ? 'trending up' : 'trending down';
  return (
    <span
      role="img"
      aria-label={label}
      style={{ fontSize: '0.8em', fontWeight: 700, color, marginLeft: '4px', lineHeight: 1 }}
    >
      {trend === 'up' ? '\u2191' : '\u2193'}
    </span>
  );
}

export function SummaryCard({
  count,
  label,
  accentColor,
  disabled,
  trend,
  improving,
}: {
  count: number | null;
  label: string;
  accentColor: string;
  disabled?: boolean;
  trend?: CardTrend;
  improving?: boolean;
}) {
  const displayCount = disabled || count === null ? '—' : String(count);
  return (
    <div
      className="crosshook-card crosshook-health-dashboard-card"
      style={{ borderLeftColor: accentColor }}
      aria-disabled={disabled}
    >
      <div className="crosshook-health-dashboard-card__count" style={{ color: disabled ? undefined : accentColor }}>
        {displayCount}
        {!disabled && trend != null && <TrendArrow trend={trend} improving={improving ?? false} />}
      </div>
      <div className="crosshook-health-dashboard-card__label crosshook-muted">{label}</div>
    </div>
  );
}

export function SkeletonCards() {
  return (
    <>
      {[0, 1, 2, 3].map((i) => (
        <div key={i} className="crosshook-card crosshook-health-dashboard-skeleton-card" aria-hidden="true">
          <div className="crosshook-health-dashboard-skeleton crosshook-health-dashboard-skeleton-card__count" />
          <div className="crosshook-health-dashboard-skeleton crosshook-health-dashboard-skeleton-card__label" />
        </div>
      ))}
    </>
  );
}
