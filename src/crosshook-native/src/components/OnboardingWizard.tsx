import { createPortal } from 'react-dom';
import { useEffect, useId, useMemo, useRef, useState, type KeyboardEvent, type MouseEvent } from 'react';

import { invoke } from '@tauri-apps/api/core';
import { ControllerPrompts } from './layout/ControllerPrompts';
import { type ProtonInstallOption, LauncherMetadataFields } from './ProfileFormSections';
import { InstallField } from './ui/InstallField';
import { ThemedSelect } from './ui/ThemedSelect';
import { ProtonPathField } from './ui/ProtonPathField';
import AutoPopulate from './AutoPopulate';
import { CustomEnvironmentVariablesSection } from './CustomEnvironmentVariablesSection';
import { useOnboarding } from '../hooks/useOnboarding';
import { useProfileContext } from '../context/ProfileContext';
import { usePreferencesContext } from '../context/PreferencesContext';
import { resolveLaunchMethod } from '../utils/launch';
import { deriveSteamClientInstallPath } from '../utils/steam';
import type { HealthIssueSeverity } from '../types/health';

export interface OnboardingWizardProps {
  open: boolean;
  mode?: 'create' | 'edit';
  onComplete: () => void;
  onDismiss: () => void;
}

const TOTAL_VISIBLE_STEPS = 3;

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

function resolveCheckIcon(severity: HealthIssueSeverity): string {
  switch (severity) {
    case 'error':
      return '✗';
    case 'warning':
      return '⚠';
    default:
      return '✓';
  }
}

function resolveCheckColor(severity: HealthIssueSeverity): string {
  switch (severity) {
    case 'error':
      return 'var(--crosshook-color-danger)';
    case 'warning':
      return 'var(--crosshook-color-warning)';
    default:
      return 'var(--crosshook-color-success)';
  }
}

function getVisibleStepNumber(isGameSetup: boolean, isTrainerSetup: boolean): number {
  if (isGameSetup) return 1;
  if (isTrainerSetup) return 2;
  return 3;
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
    readinessResult,
    checkError,
    isGameSetup,
    isTrainerSetup,
    isRuntimeSetup,
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
        const installs = await invoke<ProtonInstallOption[]>('list_proton_installs', {
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

  if (!open || !isMounted || !portalHostRef.current) {
    return null;
  }

  const visibleStep = getVisibleStepNumber(isGameSetup, isTrainerSetup);

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
            <div className="crosshook-heading-eyebrow">
              {isCompleted ? 'Complete' : `Step ${visibleStep} of ${TOTAL_VISIBLE_STEPS}`}
            </div>
            <h2 ref={headingRef} id={titleId} className="crosshook-modal__title" tabIndex={-1}>
              {isGameSetup && 'Game Setup'}
              {isTrainerSetup && 'Trainer Setup'}
              {isRuntimeSetup && 'Runtime Setup'}
              {isCompleted && 'Setup Complete'}
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
          {/* Step 1: Game Setup */}
          {isGameSetup && (
            <section aria-label="Game setup">
              {profileError && (
                <p className="crosshook-danger" role="alert" style={{ marginBottom: 12 }}>
                  {profileError}
                </p>
              )}
              <div className="crosshook-install-section-title">Profile Identity</div>
              <div className="crosshook-install-grid">
                <div className="crosshook-field">
                  <label className="crosshook-label">Profile Name</label>
                  <input
                    className="crosshook-input"
                    value={profileName}
                    placeholder="Enter a profile name"
                    onChange={(event) => setProfileName(event.target.value)}
                  />
                </div>
              </div>

              <div className="crosshook-install-section-title">Game</div>
              <div className="crosshook-install-grid">
                <div className="crosshook-field">
                  <label className="crosshook-label">Game Name</label>
                  <input
                    className="crosshook-input"
                    value={profile.game.name}
                    placeholder="God of War Ragnarok"
                    onChange={(event) =>
                      updateProfile((current) => ({
                        ...current,
                        game: { ...current.game, name: event.target.value },
                      }))
                    }
                  />
                </div>

                <InstallField
                  label="Game Path"
                  value={profile.game.executable_path}
                  onChange={(value) =>
                    updateProfile((current) => ({
                      ...current,
                      game: { ...current.game, executable_path: value },
                    }))
                  }
                  placeholder="/path/to/game.exe"
                  browseLabel="Browse"
                  browseFilters={
                    launchMethod === 'native' ? undefined : [{ name: 'Windows Executable', extensions: ['exe'] }]
                  }
                />

                <LauncherMetadataFields profile={profile} onUpdateProfile={updateProfile} />
              </div>

              <div className="crosshook-install-section-title">Runner Method</div>
              <div className="crosshook-field">
                <label className="crosshook-label">Runner Method</label>
                <ThemedSelect
                  value={launchMethod}
                  onValueChange={(val) =>
                    updateProfile((current) => ({
                      ...current,
                      steam: { ...current.steam, enabled: val === 'steam_applaunch' },
                      launch: {
                        ...current.launch,
                        method: val as typeof current.launch.method,
                      },
                    }))
                  }
                  options={[
                    { value: 'steam_applaunch', label: 'Steam app launch' },
                    { value: 'proton_run', label: 'Proton runtime launch' },
                    { value: 'native', label: 'Native Linux launch' },
                  ]}
                />
              </div>

              <CustomEnvironmentVariablesSection
                profileName={profileName}
                customEnvVars={profile.launch.custom_env_vars}
                onUpdateProfile={updateProfile}
                idPrefix="onboarding-wizard"
              />
            </section>
          )}

          {/* Step 2: Trainer Setup */}
          {isTrainerSetup && (
            <section aria-label="Trainer setup">
              <div className="crosshook-install-section-title">Trainer</div>
              <div className="crosshook-install-grid">
                <InstallField
                  label="Trainer Path"
                  value={profile.trainer.path}
                  onChange={(value) =>
                    updateProfile((current) => ({
                      ...current,
                      trainer: { ...current.trainer, path: value },
                    }))
                  }
                  placeholder="/path/to/trainer.exe"
                  browseLabel="Browse"
                  browseFilters={[{ name: 'Windows Executable', extensions: ['exe'] }]}
                />

                <div className="crosshook-field">
                  <label className="crosshook-label">Trainer Loading Mode</label>
                  <ThemedSelect
                    value={profile.trainer.loading_mode}
                    onValueChange={(value) =>
                      updateProfile((current) => ({
                        ...current,
                        trainer: {
                          ...current.trainer,
                          loading_mode: value as typeof current.trainer.loading_mode,
                        },
                      }))
                    }
                    options={[
                      { value: 'source_directory', label: 'Run from current directory' },
                      { value: 'copy_to_prefix', label: 'Copy into prefix' },
                    ]}
                  />
                </div>
              </div>
            </section>
          )}

          {/* Step 3: Runtime Setup */}
          {isRuntimeSetup && (
            <section aria-label="Runtime setup">
              {launchMethod === 'steam_applaunch' && (
                <>
                  <div className="crosshook-install-section-title">Steam Runtime</div>
                  <div className="crosshook-install-grid">
                    <div className="crosshook-field">
                      <label className="crosshook-label">Steam App ID</label>
                      <input
                        className="crosshook-input"
                        value={profile.steam.app_id}
                        placeholder="1245620"
                        onChange={(event) =>
                          updateProfile((current) => ({
                            ...current,
                            steam: { ...current.steam, app_id: event.target.value },
                          }))
                        }
                      />
                    </div>

                    <InstallField
                      label="Prefix Path"
                      value={profile.steam.compatdata_path}
                      onChange={(value) =>
                        updateProfile((current) => ({
                          ...current,
                          steam: { ...current.steam, compatdata_path: value },
                        }))
                      }
                      placeholder="/home/user/.local/share/Steam/steamapps/compatdata/1245620"
                      browseLabel="Browse"
                      browseMode="directory"
                    />
                  </div>

                  <ProtonPathField
                    value={profile.steam.proton_path}
                    onChange={(value) =>
                      updateProfile((current) => ({
                        ...current,
                        steam: { ...current.steam, proton_path: value },
                      }))
                    }
                    installs={protonInstalls}
                    installsError={protonInstallsError}
                    idPrefix="onboarding-steam"
                  />

                  <div style={{ display: 'grid', gap: 16, marginTop: 18 }}>
                    <AutoPopulate
                      gamePath={profile.game.executable_path}
                      steamClientInstallPath={deriveSteamClientInstallPath(profile.steam.compatdata_path)}
                      currentAppId={profile.steam.app_id}
                      currentCompatdataPath={profile.steam.compatdata_path}
                      currentProtonPath={profile.steam.proton_path}
                      onApplyAppId={(value) =>
                        updateProfile((current) => ({
                          ...current,
                          steam: { ...current.steam, app_id: value },
                        }))
                      }
                      onApplyCompatdataPath={(value) =>
                        updateProfile((current) => ({
                          ...current,
                          steam: { ...current.steam, compatdata_path: value },
                        }))
                      }
                      onApplyProtonPath={(value) =>
                        updateProfile((current) => ({
                          ...current,
                          steam: { ...current.steam, proton_path: value },
                        }))
                      }
                    />
                  </div>
                </>
              )}

              {launchMethod === 'proton_run' && (
                <>
                  <div className="crosshook-install-section-title">Proton Runtime</div>
                  <div className="crosshook-install-grid">
                    <InstallField
                      label="Prefix Path"
                      value={profile.runtime.prefix_path}
                      onChange={(value) =>
                        updateProfile((current) => ({
                          ...current,
                          runtime: { ...current.runtime, prefix_path: value },
                        }))
                      }
                      placeholder="/path/to/prefix"
                      browseLabel="Browse"
                      browseMode="directory"
                    />

                    <InstallField
                      label="Working Directory"
                      value={profile.runtime.working_directory}
                      onChange={(value) =>
                        updateProfile((current) => ({
                          ...current,
                          runtime: { ...current.runtime, working_directory: value },
                        }))
                      }
                      placeholder="Optional override"
                      browseLabel="Browse"
                      browseMode="directory"
                    />
                  </div>

                  <ProtonPathField
                    value={profile.runtime.proton_path}
                    onChange={(value) =>
                      updateProfile((current) => ({
                        ...current,
                        runtime: { ...current.runtime, proton_path: value },
                      }))
                    }
                    installs={protonInstalls}
                    installsError={protonInstallsError}
                    idPrefix="onboarding-proton"
                  />
                </>
              )}

              {launchMethod === 'native' && (
                <>
                  <div className="crosshook-install-section-title">Native Runtime</div>
                  <div className="crosshook-install-grid">
                    <InstallField
                      label="Working Directory"
                      value={profile.runtime.working_directory}
                      onChange={(value) =>
                        updateProfile((current) => ({
                          ...current,
                          runtime: { ...current.runtime, working_directory: value },
                        }))
                      }
                      placeholder="Optional override"
                      browseLabel="Browse"
                      browseMode="directory"
                    />
                  </div>
                </>
              )}
            </section>
          )}

          {/* Completion state */}
          {isCompleted && (
            <section aria-label="Setup complete">
              <p className="crosshook-onboarding-wizard__hint">
                Profile saved successfully. Head to the Launch page to start your game.
              </p>
            </section>
          )}

          {/* Readiness check results strip */}
          {readinessResult !== null && (
            <div className="crosshook-onboarding-wizard__checks-strip">
              <p>
                System checks:{' '}
                {readinessResult.critical_failures === 0
                  ? 'All passed'
                  : `${readinessResult.critical_failures} issue(s)`}
              </p>
              <ul>
                {readinessResult.checks.map((check) => (
                  <li key={check.field}>
                    <span style={{ color: resolveCheckColor(check.severity) }}>{resolveCheckIcon(check.severity)}</span>{' '}
                    {check.message}
                  </li>
                ))}
              </ul>
            </div>
          )}
        </div>

        {/* Footer navigation */}
        <footer className="crosshook-modal__footer crosshook-onboarding-wizard__footer">
          <div className="crosshook-onboarding-wizard__nav">
            {/* Left: Back button (hidden on step 1 and completed) */}
            {!isGameSetup && !isCompleted && (
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

              {/* Next — steps 1 and 2 */}
              {(isGameSetup || isTrainerSetup) && (
                <button
                  type="button"
                  className="crosshook-button"
                  style={{ minHeight: 'var(--crosshook-touch-target-min)' }}
                  onClick={() => advanceOrSkip(launchMethod)}
                >
                  Next
                </button>
              )}

              {/* Save Profile — step 3 only */}
              {isRuntimeSetup && (
                <button
                  type="button"
                  className="crosshook-button"
                  style={{ minHeight: 'var(--crosshook-touch-target-min)' }}
                  disabled={
                    saving || profileName.trim().length === 0 || profile.game.executable_path.trim().length === 0
                  }
                  onClick={() => void handleComplete()}
                >
                  {saving ? 'Saving...' : 'Save Profile'}
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
                  Done
                </button>
              )}
            </div>
          </div>

          <ControllerPrompts
            confirmLabel={isRuntimeSetup ? 'Save Profile' : isCompleted ? 'Done' : 'Next'}
            backLabel={isGameSetup ? 'Skip Setup' : 'Back'}
            showBumpers={false}
          />
        </footer>
      </div>
    </div>,
    portalHostRef.current
  );
}

export default OnboardingWizard;
