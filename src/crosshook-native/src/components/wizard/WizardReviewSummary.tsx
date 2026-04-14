import type { ReadinessCheckResult } from '../../types/onboarding';
import { resolveCheckColor, resolveCheckIcon } from './checkBadges';
import type { WizardValidationResult } from './wizardValidation';

export interface WizardReviewSummaryProps {
  validation: WizardValidationResult;
  readinessResult: ReadinessCheckResult | null;
  checkError: string | null;
}

/**
 * Review-step summary for the onboarding wizard.
 *
 * Renders three blocks:
 * 1. **Required Fields** — one row per `WizardRequiredField`, annotated with a
 *    satisfied / missing badge.
 * 2. **System Checks** — the most recent `runChecks()` result using the shared
 *    `resolveCheckIcon` / `resolveCheckColor` helpers. When no result has been
 *    captured yet the user is prompted to run them.
 * 3. **Tip** — a one-liner nudging the user to Back or Save from any step.
 *
 * The component is pure and does not call any IPC.
 */
export function WizardReviewSummary({ validation, readinessResult, checkError }: WizardReviewSummaryProps) {
  return (
    <div className="crosshook-onboarding-wizard__review-summary" aria-live="polite">
      <section aria-label="Required fields">
        <div className="crosshook-install-section-title">Required Fields</div>
        <ul className="crosshook-onboarding-wizard__review-list">
          {validation.fields.map((field) => {
            const badgeClass = field.isSatisfied
              ? 'crosshook-onboarding-wizard__required-badge--ok'
              : 'crosshook-onboarding-wizard__required-badge--missing';
            const glyph = field.isSatisfied ? '\u2713' : '\u2717';
            const label = field.isSatisfied ? 'Ready' : 'Missing';
            return (
              <li
                key={field.id}
                id={`wizard-review-field-${field.id}`}
                className="crosshook-onboarding-wizard__review-row"
              >
                <span
                  className={`crosshook-onboarding-wizard__required-badge ${badgeClass}`}
                  role="img"
                  aria-label={label}
                >
                  <span aria-hidden="true">{glyph}</span>
                </span>
                <span className="crosshook-onboarding-wizard__review-label">{field.label}</span>
              </li>
            );
          })}
        </ul>
      </section>

      <section aria-label="System checks" style={{ marginTop: 16 }}>
        <div className="crosshook-install-section-title">System Checks</div>
        {checkError ? (
          <p className="crosshook-danger" role="alert">
            {checkError}
          </p>
        ) : null}
        {readinessResult === null ? (
          <p className="crosshook-help-text">
            Click <strong>Run Checks</strong> to verify your environment before saving.
          </p>
        ) : (
          <>
            <p className="crosshook-help-text">
              {readinessResult.critical_failures === 0
                ? 'All checks passed.'
                : `${readinessResult.critical_failures} issue(s) found.`}
            </p>
            <ul className="crosshook-onboarding-wizard__review-list">
              {readinessResult.checks.map((check) => (
                <li
                  key={`${check.field}-${check.path}-${check.message}-${check.severity}`}
                  className="crosshook-onboarding-wizard__review-row"
                >
                  <span
                    aria-hidden="true"
                    style={{ color: resolveCheckColor(check.severity) }}
                    className="crosshook-onboarding-wizard__review-icon"
                  >
                    {resolveCheckIcon(check.severity)}
                  </span>
                  <span className="crosshook-onboarding-wizard__review-label">{check.message}</span>
                </li>
              ))}
            </ul>
          </>
        )}
      </section>

      <p className="crosshook-help-text" style={{ marginTop: 16 }}>
        Tip: Save now, or jump back to any step using <strong>Back</strong>.
      </p>
    </div>
  );
}

export default WizardReviewSummary;
