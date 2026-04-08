import './dev-indicator.css';
import type { FixtureState } from './fixture';

/** CI/release greps for this literal in built assets (`verify:no-mocks` workflow step). */
export const DEV_MODE_CI_SENTINEL = 'verify:no-mocks';

export interface DevModeBannerProps {
  /** Active fixture state from `?fixture=` (BR-11). Defaults to `populated`. */
  fixture?: FixtureState;
  /**
   * Active orthogonal debug-toggle fragments from `?errors`, `?delay`,
   * `?onboarding` (BR-12). Order is determined by `togglesToChipFragments`
   * in `lib/toggles.ts`. Empty array when no toggles are active.
   */
  toggles?: readonly string[];
}

export function DevModeBanner({ fixture = 'populated', toggles = [] }: DevModeBannerProps) {
  const label = ['DEV', fixture, ...toggles].join(' · ');
  const togglesAriaSuffix = toggles.length > 0 ? `. Toggles: ${toggles.join(', ')}` : '';
  const ariaLabel = `Browser dev mode active. Fixture: ${fixture}${togglesAriaSuffix}. ${DEV_MODE_CI_SENTINEL}`;

  return (
    <div
      className="crosshook-status-chip crosshook-status-chip--warning crosshook-dev-chip"
      role="status"
      aria-label={ariaLabel}
    >
      {label}
      <span className="crosshook-visually-hidden">{DEV_MODE_CI_SENTINEL}</span>
    </div>
  );
}
