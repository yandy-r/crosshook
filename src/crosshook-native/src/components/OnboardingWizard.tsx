import { type KeyboardEvent, type MouseEvent, useCallback, useEffect, useId, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { usePreferencesContext } from '../context/PreferencesContext';
import { useProfileContext } from '../context/ProfileContext';
import { useOnboarding } from '../hooks/useOnboarding';
import { useProtonInstalls } from '../hooks/useProtonInstalls';
import { resolveLaunchMethod } from '../utils/launch';
import { bundledOptimizationTomlKey } from '../utils/launchOptimizationPresets';
import { OnboardingIdentityStageBody } from './onboarding/OnboardingIdentityStageBody';
import { OnboardingMediaStageBody } from './onboarding/OnboardingMediaStageBody';
import { OnboardingReviewStageBody } from './onboarding/OnboardingReviewStageBody';
import { OnboardingRuntimeStageBody } from './onboarding/OnboardingRuntimeStageBody';
import { OnboardingTrainerStageBody } from './onboarding/OnboardingTrainerStageBody';
import { OnboardingWizardFooter } from './onboarding/OnboardingWizardFooter';
import { applyCreateSeed, type ProfileCreateSeed } from './wizard/profileCreateSeed';
import { getTotalVisibleSteps, getVisibleStepNumber, STAGE_TITLES } from './wizard/wizardSteps';
import { evaluateWizardRequiredFields } from './wizard/wizardValidation';

export interface OnboardingWizardProps {
  open: boolean;
  mode?: 'create' | 'edit';
  createSeed?: ProfileCreateSeed;
  onComplete: (createdName?: string) => void;
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

export function OnboardingWizard({
  open,
  mode = 'create',
  createSeed,
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
  const [nameCollisionError, setNameCollisionError] = useState<string | null>(null);
  // Captures the seed at the moment open flips true; never re-reads during the session
  // so that seed identity changes while the wizard is open do not wipe user edits.
  const capturedSeedRef = useRef<ProfileCreateSeed | undefined>(undefined);

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
    profiles,
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

  // Reset to blank profile when opening in create mode.
  // The seed is captured once when `open` flips true so that subsequent
  // identity changes to `createSeed` while the wizard is open do NOT
  // re-trigger the effect and wipe any user edits mid-session.
  // biome-ignore lint/correctness/useExhaustiveDependencies: seed/setters intentionally omitted — re-running on seed identity would wipe user edits mid-wizard
  useEffect(() => {
    if (open && mode === 'create') {
      // Snapshot the seed at open time; ignore later prop changes.
      capturedSeedRef.current = createSeed;
      let cancelled = false;

      void selectProfile('').then(() => {
        if (cancelled) return;
        const seed = capturedSeedRef.current;
        if (seed?.suggestedName) {
          setProfileName(seed.suggestedName);
        }
        if (seed) {
          updateProfile((current) => applyCreateSeed(current, seed));
        }
      });

      return () => {
        cancelled = true;
      };
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

  // biome-ignore lint/correctness/useExhaustiveDependencies: trigger-only dep — clear the collision error whenever the user edits the profile name
  useEffect(() => {
    if (nameCollisionError !== null) {
      setNameCollisionError(null);
    }
  }, [profileName]);

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

    // Duplicate-name guard (create mode only).
    if (mode === 'create' && profiles.includes(trimmedName)) {
      setNameCollisionError(`A profile named "${trimmedName}" already exists. Choose a different name.`);
      return;
    }

    const result = await persistProfileDraft(trimmedName, profile);
    if (!result.ok) return; // error is displayed via profileError from context

    setCompletedProfileName(trimmedName);
    await dismiss();
    onComplete(trimmedName);
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
          {(nameCollisionError ?? profileError) && (
            <div className="crosshook-error-banner crosshook-error-banner--section" role="alert">
              {nameCollisionError ?? profileError}
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
                {createSeed
                  ? "Profile saved successfully. It's now selected in this game's profile list."
                  : 'Profile saved successfully. Head to the Launch page to start your game.'}
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
