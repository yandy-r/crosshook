import type { HealthIssueSeverity } from '../../types/health';

/**
 * Resolve the glyph used to annotate a health/readiness check row based on its
 * severity. Shared between `OnboardingWizard.tsx` and `WizardReviewSummary.tsx`
 * so both surfaces cannot drift in labelling.
 */
export function resolveCheckIcon(severity: HealthIssueSeverity): string {
  switch (severity) {
    case 'error':
      return '\u2717'; // ✗
    case 'warning':
      return '\u26A0'; // ⚠
    case 'info':
      return '\u2139'; // ℹ
    default:
      return '\u2713'; // ✓
  }
}

/**
 * Resolve the CSS variable-based colour used to annotate a health/readiness
 * check row based on its severity. All values must remain design-token driven —
 * no hardcoded hex values.
 */
export function resolveCheckColor(severity: HealthIssueSeverity): string {
  switch (severity) {
    case 'error':
      return 'var(--crosshook-color-danger)';
    case 'warning':
      return 'var(--crosshook-color-warning)';
    case 'info':
      return 'var(--crosshook-color-info)';
    default:
      return 'var(--crosshook-color-success)';
  }
}
