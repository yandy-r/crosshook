import './dev-indicator.css';

/** CI/release greps for this literal in built assets (`verify:no-mocks` workflow step). */
export const DEV_MODE_CI_SENTINEL = 'verify:no-mocks';

export interface DevModeBannerProps {
  fixture?: string; // Phase 3 will pass the active fixture name; Phase 1 always 'populated'
}

export function DevModeBanner({ fixture = 'populated' }: DevModeBannerProps) {
  return (
    <div
      className="crosshook-status-chip crosshook-status-chip--warning crosshook-dev-chip"
      role="status"
      aria-label={`Browser dev mode active. Fixture: ${fixture}. ${DEV_MODE_CI_SENTINEL}`}
    >
      DEV · {fixture}
      <span className="crosshook-visually-hidden">{DEV_MODE_CI_SENTINEL}</span>
    </div>
  );
}
