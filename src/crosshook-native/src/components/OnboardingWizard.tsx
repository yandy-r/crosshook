import { type KeyboardEvent, type MouseEvent, useCallback, useEffect, useId, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { usePreferencesContext } from '../context/PreferencesContext';
import { useProfileContext } from '../context/ProfileContext';
import { useOnboarding } from '../hooks/useOnboarding';
import { useProtonInstalls } from '../hooks/useProtonInstalls';
import type { ResolvedLaunchMethod } from '../types';
import type { OnboardingWizardStage } from '../types/onboarding';
import { resolveLaunchMethod } from '../utils/launch';
import { bundledOptimizationTomlKey } from '../utils/launchOptimizationPresets';
import { OnboardingIdentityStageBody } from './onboarding/OnboardingIdentityStageBody';
import { OnboardingMediaStageBody } from './onboarding/OnboardingMediaStageBody';
import { OnboardingReviewStageBody } from './onboarding/OnboardingReviewStageBody';
import { OnboardingRuntimeStageBody } from './onboarding/OnboardingRuntimeStageBody';
import { OnboardingTrainerStageBody } from './onboarding/OnboardingTrainerStageBody';
import { OnboardingWizardFooter } from './onboarding/OnboardingWizardFooter';
import { evaluateWizardRequiredFields } from './wizard/wizardValidation';

export interface OnboardingWizardProps {
  open: boolean;
  mode?: 'create' | 'edit';
  onComplete: () => void;
  onDismiss: () => void;
  onOpenHostToolDashboard?: () => void;
}

const FOCUSABLE_SELECTOR = [
  'a[href]',
  'button:not([disabled])',
  'input:not([disabled]):not([type="hidden"])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
  '[contenteditable="true"]',
].join(', ');

function getFocusableElements(container: HTMLElement): HTMLElement[] {
  return Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(
    (element) => !element.hasAttribute('disabled') && element.tabIndex >= 0 && element.getClientRects().length > 0
  );
}

function focusElement(element: HTMLElement | null): boolean {
  if (!element) return false;
  element.focus({ preventScroll: true });
  return document.activeElement === element;
}

const STAGE_TITLES: Record<OnboardingWizardStage, string> = {
  identity_game: 'Identity & Game',
  runtime: 'Runtime',
  trainer: 'Trainer',
  media: 'Media',
  review: 'Review & Save',
  completed: 'Setup Complete',
};

/** Returns the 1-based step number shown in the eyebrow (native skips trainer, shifting later stages). */
function getVisibleStepNumber(stage: OnboardingWizardStage, launchMethod: ResolvedLaunchMethod): number {
  const skipsTrainer = launchMethod === 'native';
  switch (stage) {
    case 'identity_game':
      return 1;
    case 'runtime':
      return 2;
    case 'trainer':
      return 3;
    case 'media':
      return skipsTrainer ? 3 : 4;
    case 'review':
      return skipsTrainer ? 4 : 5;
    case 'completed':
      return skipsTrainer ? 4 : 5;
  }
}

function getTotalVisibleSteps(launchMethod: ResolvedLaunchMethod): number {
  return launchMethod === 'native' ? 4 : 5;
}

export function OnboardingWizard({
  open,
  mode = 'create',
  onComplete,
  onDismiss,
  onOpenHostToolDashboard,
}: OnboardingWizardProps) {
  const portalHostRef = useRef<HTMLElement | null>(null);
  const surfaceRef = useRef<HTMLDivElement | null>(null);
  const headingRef = useRef<HTMLHeadingElement | null>(null);
  const previouslyFocusedRef = useRef<HTMLElement | null>(null);
  const bodyStyleRef = useRef<string>('');
  const hiddenNodesRef = useRef<Array<{ element: HTMLElement; inert: boolean; ariaHidden: string | null }>>([]);
  const titleId = useId();
  const [isMounted, setIsMounted] = useState(false);

  const {
    stage,
    readinessResult,
    checkError,
    isRunningChecks,
    lastCheckedAt,
    umuInstallGuidance,
    steamDeckCaveats,
    isIdentityGame,
    isRuntime,
    isTrainer,
    isMedia,
    isReview,
    isCompleted,
    runChecks,
    advanceOrSkip,
    goBack,
    dismiss,
    dismissUmuInstallNag,
    dismissSteamDeckCaveats,
    dismissReadinessNag,
    setCompletedProfileName,
  } = useOnboarding();

  const { defaultSteamClientInstallPath } = usePreferencesContext();
  const {
    profileName,
    profile,
    saving,
    error: profileError,
    setProfileName,
    updateProfile,
    persistProfileDraft,
    selectProfile,
    steamClientInstallPath,
    bundledOptimizationPresets,
    applyBundledOptimizationPreset,
    switchLaunchOptimizationPreset,
    optimizationPresetActionBusy,
  } = useProfileContext();

  // Reset to blank profile when opening in create mode
  useEffect(() => {
    if (open && mode === 'create') {
      void selectProfile('');
    }
  }, [open, mode, selectProfile]);

  const launchMethod = resolveLaunchMethod(profile);

  const effectiveSteamClientInstallPath = useMemo(
    () => defaultSteamClientInstallPath || steamClientInstallPath,
    [defaultSteamClientInstallPath, steamClientInstallPath]
  );

  const { installs: protonInstalls, error: protonInstallsError } = useProtonInstalls({
    steamClientInstallPath: effectiveSteamClientInstallPath,
  });

  // Portal host — created unconditionally on mount, not gated on `open`
  useEffect(() => {
    if (typeof document === 'undefined') return;

    const host = document.createElement('div');
    host.className = 'crosshook-modal-portal';
    portalHostRef.current = host;
    document.body.appendChild(host);
    setIsMounted(true);

    return () => {
      host.remove();
      portalHostRef.current = null;
      setIsMounted(false);
    };
  }, []);

  // Inert siblings + overflow lock + focus management when open
  useEffect(() => {
    if (!open || typeof document === 'undefined') return;

    const { body } = document;
    const portalHost = portalHostRef.current;
    if (!portalHost) return;

    previouslyFocusedRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;

    bodyStyleRef.current = body.style.overflow;
    body.style.overflow = 'hidden';
    body.classList.add('crosshook-modal-open');

    hiddenNodesRef.current = Array.from(body.children)
      .filter((child): child is HTMLElement => child instanceof HTMLElement && child !== portalHost)
      .map((element) => {
        const inertState = (element as HTMLElement & { inert?: boolean }).inert ?? false;
        const ariaHidden = element.getAttribute('aria-hidden');
        (element as HTMLElement & { inert?: boolean }).inert = true;
        element.setAttribute('aria-hidden', 'true');
        return { element, inert: inertState, ariaHidden };
      });

    const frame = window.requestAnimationFrame(() => {
      if (!focusElement(headingRef.current)) {
        const focusable = surfaceRef.current ? getFocusableElements(surfaceRef.current) : [];
        if (focusable.length > 0) focusElement(focusable[0]);
      }
    });

    return () => {
      window.cancelAnimationFrame(frame);
      body.style.overflow = bodyStyleRef.current;
      body.classList.remove('crosshook-modal-open');

      for (const { element, inert, ariaHidden } of hiddenNodesRef.current) {
        (element as HTMLElement & { inert?: boolean }).inert = inert;
        if (ariaHidden === null) {
          element.removeAttribute('aria-hidden');
        } else {
          element.setAttribute('aria-hidden', ariaHidden);
        }
      }
      hiddenNodesRef.current = [];

      const restoreTarget = previouslyFocusedRef.current;
      if (restoreTarget?.isConnected) focusElement(restoreTarget);
      previouslyFocusedRef.current = null;
    };
  }, [open]);

  const validation = useMemo(
    () => evaluateWizardRequiredFields({ profileName, profile, launchMethod }),
    [profileName, profile, launchMethod]
  );

  // Create: mutate draft in-memory (IPC rejects unsaved profiles). Edit: IPC path for server-side revision tracking.
  const applyBundledPresetToDraft = useCallback(
    async (presetId: string): Promise<void> => {
      const preset = bundledOptimizationPresets.find((candidate) => candidate.preset_id === presetId);
      if (!preset) return;
      const key = bundledOptimizationTomlKey(preset.preset_id);
      updateProfile((current) => ({
        ...current,
        launch: {
          ...current.launch,
          optimizations: {
            ...current.launch.optimizations,
            enabled_option_ids: [...preset.enabled_option_ids],
          },
          presets: {
            ...(current.launch.presets ?? {}),
            [key]: {
              enabled_option_ids: [...preset.enabled_option_ids],
            },
          },
          active_preset: key,
        },
      }));
    },
    [bundledOptimizationPresets, updateProfile]
  );

  const applySavedPresetToDraft = useCallback(
    async (presetName: string): Promise<void> => {
      const trimmed = presetName.trim();
      if (trimmed.length === 0) return;
      const target = profile.launch.presets?.[trimmed];
      if (!target) return;
      updateProfile((current) => ({
        ...current,
        launch: {
          ...current.launch,
          optimizations: {
            ...current.launch.optimizations,
            enabled_option_ids: [...(target.enabled_option_ids ?? [])],
          },
          active_preset: trimmed,
        },
      }));
    },
    [profile.launch.presets, updateProfile]
  );

  const onApplyBundledPreset = mode === 'edit' ? applyBundledOptimizationPreset : applyBundledPresetToDraft;
  const onSelectSavedPreset = mode === 'edit' ? switchLaunchOptimizationPreset : applySavedPresetToDraft;

  function handleKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === 'Escape') {
      event.stopPropagation();
      event.preventDefault();
      void handleSkip();
      return;
    }

    if (event.key !== 'Tab') return;

    if (!surfaceRef.current) return;
    const focusable = getFocusableElements(surfaceRef.current);
    if (focusable.length === 0) {
      event.preventDefault();
      focusElement(headingRef.current);
      return;
    }

    const currentIndex = focusable.indexOf(document.activeElement as HTMLElement);
    const lastIndex = focusable.length - 1;

    if (event.shiftKey) {
      if (currentIndex <= 0) {
        event.preventDefault();
        focusElement(focusable[lastIndex]);
      }
      return;
    }

    if (currentIndex === -1 || currentIndex === lastIndex) {
      event.preventDefault();
      focusElement(focusable[0]);
    }
  }

  function handleBackdropMouseDown(event: MouseEvent<HTMLDivElement>) {
    // Wizard requires explicit user action to dismiss — backdrop click is intentionally ignored
    if (event.target !== event.currentTarget) return;
  }

  async function handleSkip() {
    await dismiss();
    onDismiss();
  }

  async function handleComplete() {
    const trimmedName = profileName.trim();
    if (trimmedName.length === 0) return;

    const result = await persistProfileDraft(trimmedName, profile);
    if (!result.ok) return; // error is displayed via profileError from context

    setCompletedProfileName(trimmedName);
    await dismiss();
    onComplete();
  }

  function handleBack() {
    goBack(launchMethod);
  }

  function handleNext() {
    advanceOrSkip(launchMethod);
  }

  function handleOpenHostToolDashboard() {
    onOpenHostToolDashboard?.();
  }

  if (!open || !isMounted || !portalHostRef.current) {
    return null;
  }

  const visibleStep = getVisibleStepNumber(stage, launchMethod);
  const totalVisibleSteps = getTotalVisibleSteps(launchMethod);
  const title = STAGE_TITLES[stage];
  const eyebrow = isCompleted ? 'Complete' : `Step ${visibleStep} of ${totalVisibleSteps}`;
  const confirmLabel = isCompleted ? 'Done' : isReview ? (saving ? 'Saving...' : 'Save Profile') : 'Next';
  const saveDescribedBy =
    !validation.isReady && validation.firstMissingId !== null
      ? `wizard-review-field-${validation.firstMissingId}`
      : undefined;

  return createPortal(
    <div className="crosshook-modal" role="presentation">
      <div className="crosshook-modal__backdrop" aria-hidden="true" onMouseDown={handleBackdropMouseDown} />
      <div
        ref={surfaceRef}
        className="crosshook-modal__surface crosshook-panel crosshook-focus-scope crosshook-onboarding-wizard"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        data-crosshook-focus-root="modal"
        onKeyDown={handleKeyDown}
      >
        <header className="crosshook-modal__header">
          <div className="crosshook-modal__heading-block">
            <p className="crosshook-heading-eyebrow">{eyebrow}</p>
            <h2
              ref={headingRef}
              id={titleId}
              className="crosshook-heading-title crosshook-heading-title--card"
              tabIndex={-1}
            >
              {title}
            </h2>
          </div>

          {!isCompleted && (
            <div className="crosshook-modal__header-actions">
              <button
                type="button"
                className="crosshook-button crosshook-button--ghost crosshook-modal__close"
                onClick={() => void handleSkip()}
                data-crosshook-modal-close
              >
                Skip Setup
              </button>
            </div>
          )}
        </header>

        <div className="crosshook-modal__body crosshook-onboarding-wizard__body">
          {profileError && (
            <div className="crosshook-error-banner crosshook-error-banner--section" role="alert">
              {profileError}
            </div>
          )}

          {isIdentityGame && (
            <OnboardingIdentityStageBody
              profile={profile}
              profileName={profileName}
              launchMethod={launchMethod}
              profileExists={mode === 'edit'}
              onProfileNameChange={setProfileName}
              onUpdateProfile={updateProfile}
            />
          )}

          {isRuntime && (
            <OnboardingRuntimeStageBody
              profile={profile}
              launchMethod={launchMethod}
              protonInstalls={protonInstalls}
              protonInstallsError={protonInstallsError}
              onUpdateProfile={updateProfile}
              onOpenHostToolDashboard={onOpenHostToolDashboard ? handleOpenHostToolDashboard : undefined}
            />
          )}

          {isTrainer && (
            <OnboardingTrainerStageBody
              profile={profile}
              profileName={profileName}
              launchMethod={launchMethod}
              profileExists={mode === 'edit'}
              onUpdateProfile={updateProfile}
            />
          )}

          {isMedia && (
            <OnboardingMediaStageBody profile={profile} launchMethod={launchMethod} onUpdateProfile={updateProfile} />
          )}

          {isReview && (
            <OnboardingReviewStageBody
              profile={profile}
              profileName={profileName}
              mode={mode}
              validation={validation}
              bundledOptimizationPresets={bundledOptimizationPresets}
              optimizationPresetActionBusy={optimizationPresetActionBusy}
              readinessResult={readinessResult}
              checkError={checkError}
              umuInstallGuidance={umuInstallGuidance}
              steamDeckCaveats={steamDeckCaveats}
              onUpdateProfile={updateProfile}
              onApplyBundledPreset={onApplyBundledPreset}
              onSelectSavedPreset={onSelectSavedPreset}
              onDismissUmuInstallNag={() => void dismissUmuInstallNag()}
              onDismissSteamDeckCaveats={() => void dismissSteamDeckCaveats()}
              onDismissReadinessNag={(toolId) => void dismissReadinessNag(toolId)}
              onOpenHostToolDashboard={onOpenHostToolDashboard ? handleOpenHostToolDashboard : undefined}
            />
          )}

          {isCompleted && (
            <section aria-label="Setup complete">
              <p className="crosshook-onboarding-wizard__hint">
                Profile saved successfully. Head to the Launch page to start your game.
              </p>
            </section>
          )}
        </div>

        <OnboardingWizardFooter
          confirmLabel={confirmLabel}
          isIdentityGame={isIdentityGame}
          isRuntime={isRuntime}
          isTrainer={isTrainer}
          isMedia={isMedia}
          isReview={isReview}
          isCompleted={isCompleted}
          isRunningChecks={isRunningChecks}
          isSaving={saving}
          isSaveReady={validation.isReady}
          lastCheckedAt={lastCheckedAt}
          saveDescribedBy={saveDescribedBy}
          onBack={handleBack}
          onNext={handleNext}
          onComplete={isCompleted ? onComplete : () => void handleComplete()}
          onRunChecks={() => void runChecks()}
        />
      </div>
    </div>,
    portalHostRef.current
  );
}
export default OnboardingWizard;
