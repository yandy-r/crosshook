import { ControllerPrompts } from '../layout/ControllerPrompts';

export interface OnboardingWizardFooterProps {
  confirmLabel: string;
  isIdentityGame: boolean;
  isRuntime: boolean;
  isTrainer: boolean;
  isMedia: boolean;
  isReview: boolean;
  isCompleted: boolean;
  isRunningChecks: boolean;
  isSaving: boolean;
  isSaveReady: boolean;
  lastCheckedAt: string | null;
  saveDescribedBy: string | undefined;
  onBack: () => void;
  onNext: () => void;
  onComplete: () => void;
  onRunChecks: () => void;
}

export function OnboardingWizardFooter({
  confirmLabel,
  isIdentityGame,
  isRuntime,
  isTrainer,
  isMedia,
  isReview,
  isCompleted,
  isRunningChecks,
  isSaving,
  isSaveReady,
  lastCheckedAt,
  saveDescribedBy,
  onBack,
  onNext,
  onComplete,
  onRunChecks,
}: OnboardingWizardFooterProps) {
  return (
    <footer className="crosshook-modal__footer crosshook-onboarding-wizard__footer">
      <div className="crosshook-onboarding-wizard__nav">
        {/* Left: Back button (hidden on first step and completed) */}
        {!isIdentityGame && !isCompleted && (
          <button
            type="button"
            className="crosshook-button crosshook-button--secondary"
            style={{ minHeight: 'var(--crosshook-touch-target-min)' }}
            onClick={onBack}
          >
            Back
          </button>
        )}

        <div className="crosshook-onboarding-wizard__nav-primary">
          {/* Run Checks — always available except on completed */}
          {!isCompleted && (
            <div className="crosshook-onboarding-wizard__checks-action">
              <button
                type="button"
                className="crosshook-button crosshook-button--secondary"
                style={{ minHeight: 'var(--crosshook-touch-target-min)' }}
                disabled={isRunningChecks}
                onClick={onRunChecks}
              >
                {isRunningChecks ? 'Running Checks...' : 'Run Checks'}
              </button>
              {!isRunningChecks && lastCheckedAt ? (
                <span className="crosshook-help-text" aria-live="polite" style={{ marginLeft: 8 }}>
                  Last checked at {lastCheckedAt}
                </span>
              ) : null}
            </div>
          )}

          {/* Next — steps 1–4 */}
          {(isIdentityGame || isRuntime || isTrainer || isMedia) && (
            <button
              type="button"
              className="crosshook-button"
              style={{ minHeight: 'var(--crosshook-touch-target-min)' }}
              onClick={onNext}
            >
              {confirmLabel}
            </button>
          )}

          {/* Save Profile — review step only */}
          {isReview && (
            <button
              type="button"
              className="crosshook-button"
              style={{ minHeight: 'var(--crosshook-touch-target-min)' }}
              disabled={isSaving || !isSaveReady}
              aria-describedby={saveDescribedBy}
              onClick={onComplete}
            >
              {confirmLabel}
            </button>
          )}

          {/* Done — completed state */}
          {isCompleted && (
            <button
              type="button"
              className="crosshook-button"
              style={{ minHeight: 'var(--crosshook-touch-target-min)' }}
              onClick={onComplete}
            >
              {confirmLabel}
            </button>
          )}
        </div>
      </div>

      <ControllerPrompts
        confirmLabel={confirmLabel}
        backLabel={isIdentityGame ? 'Skip Setup' : 'Back'}
        showBumpers={false}
      />
    </footer>
  );
}
