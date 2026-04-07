import './dev-indicator.css';

export interface DevModeBannerProps {
  fixture?: string; // Phase 3 will pass the active fixture name; Phase 1 always 'populated'
}

export function DevModeBanner({ fixture = 'populated' }: DevModeBannerProps) {
  return (
    <div
      className="crosshook-status-chip crosshook-status-chip--warning crosshook-dev-chip"
      role="status"
      aria-label={`Browser dev mode active. Fixture: ${fixture}`}
    >
      DEV · {fixture}
    </div>
  );
}
