import { createPortal } from 'react-dom';
import {
  useCallback,
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type MouseEvent,
} from 'react';

import { callCommand } from '@/lib/ipc';
import { ControllerPrompts } from './layout/ControllerPrompts';
import { CustomEnvironmentVariablesSection } from './CustomEnvironmentVariablesSection';
import { ProfileIdentitySection } from './profile-sections/ProfileIdentitySection';
import { GameSection } from './profile-sections/GameSection';
import { RunnerMethodSection } from './profile-sections/RunnerMethodSection';
import { RuntimeSection } from './profile-sections/RuntimeSection';
import { TrainerSection } from './profile-sections/TrainerSection';
import { MediaSection } from './profile-sections/MediaSection';
import { WizardPresetPicker } from './wizard/WizardPresetPicker';
import { WizardReviewSummary } from './wizard/WizardReviewSummary';
import { evaluateWizardRequiredFields } from './wizard/wizardValidation';
import { useOnboarding } from '../hooks/useOnboarding';
import { useProfileContext } from '../context/ProfileContext';
import { usePreferencesContext } from '../context/PreferencesContext';
import { resolveLaunchMethod } from '../utils/launch';
import { bundledOptimizationTomlKey } from '../utils/launchOptimizationPresets';
import type { OnboardingWizardStage } from '../types/onboarding';
import type { ProtonInstallOption } from '../types/proton';
import type { ResolvedLaunchMethod } from '../types';

export interface OnboardingWizardProps {
  open: boolean;
  mode?: 'create' | 'edit';
  onComplete: () => void;
  onDismiss: () => void;
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

/**
 * Resolves the visible step number shown in the header eyebrow given the
 * current stage and launch method. The trainer stage is skipped for native
 * profiles; Media / Review slide up by one position as a result.
 */
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

export function OnboardingWizard({ open, mode = 'create', onComplete, onDismiss }: OnboardingWizardProps) {
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

  const [protonInstalls, setProtonInstalls] = useState<ProtonInstallOption[]>([]);
  const [protonInstallsError, setProtonInstallsError] = useState<string | null>(null);

  const effectiveSteamClientInstallPath = useMemo(
    () => defaultSteamClientInstallPath || steamClientInstallPath,
    [defaultSteamClientInstallPath, steamClientInstallPath]
  );

  useEffect(() => {
    let active = true;
    async function loadProtonInstalls() {
      try {
        const installs = await callCommand<ProtonInstallOption[]>('list_proton_installs', {
          steamClientInstallPath:
            effectiveSteamClientInstallPath.trim().length > 0 ? effectiveSteamClientInstallPath : undefined,
        });
        if (active) {
          setProtonInstalls(installs);
          setProtonInstallsError(null);
        }
      } catch (err) {
        if (active) setProtonInstallsError(String(err));
      }
    }
    void loadProtonInstalls();
    return () => {
      active = false;
    };
  }, [effectiveSteamClientInstallPath]);

  // Portal host — created unconditionally on mount, NOT gated on `open`
  // (following ProfileReviewModal.tsx pattern exactly)
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
      if (restoreTarget && restoreTarget.isConnected) focusElement(restoreTarget);
      previouslyFocusedRef.current = null;
    };
  }, [open]);

  const validation = useMemo(
    () => evaluateWizardRequiredFields({ profileName, profile, launchMethod }),
    [profileName, profile, launchMethod]
  );

  // In create mode, the draft profile has not been persisted yet, so the
  // backend preset IPCs (applyBundledOptimizationPreset /
  // switchLaunchOptimizationPreset) refuse to run because
  // hasExistingSavedProfile === false. We instead mutate the draft profile
  // in-memory and let persistProfileDraft persist the optimizations, the
  // [launch.presets.<key>] entry, and the active_preset in one transaction.
  // Edit mode keeps the IPC-based path so config-revision capture and preset
  // metadata origin tracking continue to run server-side.
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

  if (!open || !isMounted || !portalHostRef.current) {
    return null;
  }

  const visibleStep = getVisibleStepNumber(stage, launchMethod);
  const totalVisibleSteps = getTotalVisibleSteps(launchMethod);
  const title = STAGE_TITLES[stage];
  const eyebrow = isCompleted ? 'Complete' : `Step ${visibleStep} of ${totalVisibleSteps}`;
  const confirmLabel = isCompleted
    ? 'Done'
    : isReview
      ? saving
        ? 'Saving...'
        : 'Save Profile'
      : 'Next';
  const saveDescribedBy = !validation.isReady && validation.firstMissingId !== null
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
        {/* Header */}
        <header className="crosshook-modal__header">
          <div className="crosshook-modal__heading-block">
            <div className="crosshook-heading-eyebrow">{eyebrow}</div>
            <h2 ref={headingRef} id={titleId} className="crosshook-modal__title" tabIndex={-1}>
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

        {/* Step content */}
        <div className="crosshook-modal__body crosshook-onboarding-wizard__body">
          {profileError && (
            <p className="crosshook-danger" role="alert" style={{ marginBottom: 12 }}>
              {profileError}
            </p>
          )}

          {isIdentityGame && (
            <section aria-label="Identity & Game" className="crosshook-onboarding-wizard__step-grid">
              <ProfileIdentitySection
                profileName={profileName}
                profile={profile}
                onProfileNameChange={setProfileName}
                onUpdateProfile={updateProfile}
                profileExists={mode === 'edit'}
              />
              <GameSection profile={profile} onUpdateProfile={updateProfile} launchMethod={launchMethod} />
              <RunnerMethodSection profile={profile} onUpdateProfile={updateProfile} />
            </section>
          )}

          {isRuntime && (
            <section aria-label="Runtime" className="crosshook-onboarding-wizard__step-grid">
              <RuntimeSection
                profile={profile}
                onUpdateProfile={updateProfile}
                launchMethod={launchMethod}
                protonInstalls={protonInstalls}
                protonInstallsError={protonInstallsError}
              />
            </section>
          )}

          {isTrainer && (
            <section aria-label="Trainer" className="crosshook-onboarding-wizard__step-grid">
              <TrainerSection
                profile={profile}
                onUpdateProfile={updateProfile}
                launchMethod={launchMethod}
                profileName={profileName}
                profileExists={mode === 'edit'}
              />
            </section>
          )}

          {isMedia && (
            <section aria-label="Media" className="crosshook-onboarding-wizard__step-grid">
              <MediaSection profile={profile} onUpdateProfile={updateProfile} launchMethod={launchMethod} />
            </section>
          )}

          {isReview && (
            <section aria-label="Review & Save" className="crosshook-onboarding-wizard__step-grid">
              <WizardPresetPicker
                bundledPresets={bundledOptimizationPresets}
                savedPresetNames={Object.keys(profile.launch.presets ?? {})}
                activePresetKey={profile.launch.active_preset ?? ''}
                busy={mode === 'edit' ? optimizationPresetActionBusy : false}
                onApplyBundled={onApplyBundledPreset}
                onSelectSaved={onSelectSavedPreset}
              />
              <CustomEnvironmentVariablesSection
                profileName={profileName}
                customEnvVars={profile.launch.custom_env_vars}
                onUpdateProfile={updateProfile}
                idPrefix="onboarding-wizard"
              />
              <WizardReviewSummary
                validation={validation}
                readinessResult={readinessResult}
                checkError={checkError}
              />
            </section>
          )}

          {isCompleted && (
            <section aria-label="Setup complete">
              <p className="crosshook-onboarding-wizard__hint">
                Profile saved successfully. Head to the Launch page to start your game.
              </p>
            </section>
          )}
        </div>

        {/* Footer navigation */}
        <footer className="crosshook-modal__footer crosshook-onboarding-wizard__footer">
          <div className="crosshook-onboarding-wizard__nav">
            {/* Left: Back button (hidden on first step and completed) */}
            {!isIdentityGame && !isCompleted && (
              <button
                type="button"
                className="crosshook-button crosshook-button--secondary"
                style={{ minHeight: 'var(--crosshook-touch-target-min)' }}
                onClick={handleBack}
              >
                Back
              </button>
            )}

            <div className="crosshook-onboarding-wizard__nav-primary">
              {/* Run Checks — always available except on completed */}
              {!isCompleted && (
                <button
                  type="button"
                  className="crosshook-button crosshook-button--secondary"
                  style={{ minHeight: 'var(--crosshook-touch-target-min)' }}
                  onClick={() => void runChecks()}
                >
                  Run Checks
                </button>
              )}

              {/* Next — steps 1–4 */}
              {(isIdentityGame || isRuntime || isTrainer || isMedia) && (
                <button
                  type="button"
                  className="crosshook-button"
                  style={{ minHeight: 'var(--crosshook-touch-target-min)' }}
                  onClick={handleNext}
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
                  disabled={saving || !validation.isReady}
                  aria-describedby={saveDescribedBy}
                  onClick={() => void handleComplete()}
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
      </div>
    </div>,
    portalHostRef.current
  );
}

export default OnboardingWizard;
