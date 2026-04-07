import { useCallback, useEffect, useId, useMemo, useRef, useState } from 'react';
import * as Tabs from '@radix-ui/react-tabs';

import { CustomEnvironmentVariablesSection } from './CustomEnvironmentVariablesSection';
import { InstallField } from './ui/InstallField';
import { InstallReviewSummary } from './install/InstallReviewSummary';
import { ProfileIdentitySection } from './profile-sections/ProfileIdentitySection';
import { RunnerMethodSection } from './profile-sections/RunnerMethodSection';
import { RuntimeSection } from './profile-sections/RuntimeSection';
import { TrainerSection } from './profile-sections/TrainerSection';
import { MediaSection } from './profile-sections/MediaSection';
import { WizardPresetPicker } from './wizard/WizardPresetPicker';
import { useInstallGame } from '../hooks/useInstallGame';
import { useProtonInstalls } from '../hooks/useProtonInstalls';
import { useProfileContext } from '../context/ProfileContext';
import type { GameProfile } from '../types/profile';
import type { InstallProfileReviewPayload, ProfileReviewSource } from '../types/install';
import { resolveLaunchMethod } from '../utils/launch';
import { bundledOptimizationTomlKey } from '../utils/launchOptimizationPresets';
import { evaluateInstallRequiredFields } from './install/installValidation';

type InstallFlowTabId = 'identity' | 'runtime' | 'trainer' | 'media' | 'installer_review';

const INSTALL_FLOW_TAB_LABELS: Record<InstallFlowTabId, string> = {
  identity: 'Identity & Game',
  runtime: 'Runtime',
  trainer: 'Trainer',
  media: 'Media',
  installer_review: 'Installer & Review',
};

function isInstallFlowTabId(value: string): value is InstallFlowTabId {
  return Object.prototype.hasOwnProperty.call(INSTALL_FLOW_TAB_LABELS, value);
}

function prefixStateLabel(state: import('../types/install').InstallGamePrefixPathState): string {
  switch (state) {
    case 'loading':
      return 'Resolving default prefix...';
    case 'ready':
      return 'Default prefix resolved';
    case 'failed':
      return 'Default prefix unavailable';
    case 'idle':
    default:
      return 'Awaiting profile name';
  }
}

export interface InstallGamePanelProps {
  onOpenProfileReview: (payload: InstallProfileReviewPayload) => void | Promise<boolean>;
  onRequestInstallAction?: (action: 'retry' | 'reset') => boolean | Promise<boolean>;
}

export function InstallGamePanel({ onOpenProfileReview, onRequestInstallAction }: InstallGamePanelProps) {
  const {
    profileName,
    setProfileName,
    draftProfile,
    updateDraftProfile,
    installerInputs,
    updateInstallerInputs,
    validation,
    stage,
    result,
    reviewProfile,
    error,
    defaultPrefixPath,
    defaultPrefixPathState,
    defaultPrefixPathError,
    candidateOptions,
    isRunningInstaller,
    isResolvingDefaultPrefixPath,
    setInstalledExecutablePath,
    startInstall,
    reset,
    actionLabel,
    statusText,
    hintText,
  } = useInstallGame();

  const { bundledOptimizationPresets } = useProfileContext();
  const { installs: protonInstalls, error: protonInstallsError } = useProtonInstalls();

  const launchMethod = resolveLaunchMethod(draftProfile);
  const installFlowTabs = useMemo(() => {
    const all: InstallFlowTabId[] = ['identity', 'runtime', 'trainer', 'media', 'installer_review'];
    const visible = launchMethod === 'native' ? all.filter((id) => id !== 'trainer') : all;
    return visible.map((id) => ({ id, label: INSTALL_FLOW_TAB_LABELS[id] }));
  }, [launchMethod]);

  const [activeInstallTab, setActiveInstallTab] = useState<InstallFlowTabId>('identity');
  const installRequiredHintId = useId();

  useEffect(() => {
    if (launchMethod === 'native' && activeInstallTab === 'trainer') {
      setActiveInstallTab('media');
    }
  }, [launchMethod, activeInstallTab]);

  useEffect(() => {
    const ids = installFlowTabs.map((t) => t.id);
    if (!ids.includes(activeInstallTab)) {
      setActiveInstallTab(ids.includes('identity') ? 'identity' : ids[0] ?? 'identity');
    }
  }, [installFlowTabs, activeInstallTab]);

  const installValidation = useMemo(
    () =>
      evaluateInstallRequiredFields({
        profileName,
        profile: draftProfile,
        launchMethod,
        installerPath: installerInputs.installer_path,
      }),
    [profileName, draftProfile, launchMethod, installerInputs.installer_path]
  );

  const applyBundledPresetToDraft = useCallback(
    async (presetId: string): Promise<void> => {
      const preset = bundledOptimizationPresets.find((candidate) => candidate.preset_id === presetId);
      if (!preset) return;
      const key = bundledOptimizationTomlKey(preset.preset_id);
      updateDraftProfile((current) => ({
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
    [bundledOptimizationPresets, updateDraftProfile]
  );

  const applySavedPresetToDraft = useCallback(
    async (presetName: string): Promise<void> => {
      const trimmed = presetName.trim();
      if (trimmed.length === 0) return;
      updateDraftProfile((current) => {
        const target = current.launch.presets?.[trimmed];
        if (!target) return current;
        return {
          ...current,
          launch: {
            ...current.launch,
            optimizations: {
              ...current.launch.optimizations,
              enabled_option_ids: [...(target.enabled_option_ids ?? [])],
            },
            active_preset: trimmed,
          },
        };
      });
    },
    [updateDraftProfile]
  );

  const logPath = result?.helper_log_path ?? '';
  const reviewableInstallResult = result?.succeeded === true && reviewProfile !== null ? result : null;
  const canReviewGeneratedProfile = reviewableInstallResult !== null && reviewProfile !== null;
  const lastAutoOpenReviewKeyRef = useRef<string | null>(null);

  const openReviewPayload = useCallback(
    (source: ProfileReviewSource) => {
      if (reviewableInstallResult === null || reviewProfile === null) {
        return;
      }

      const generatedProfile: GameProfile = { ...reviewProfile };

      void onOpenProfileReview({
        source,
        profileName: reviewableInstallResult.profile_name.trim() || profileName.trim(),
        generatedProfile,
        candidateOptions,
        helperLogPath: logPath,
        message: reviewableInstallResult.message,
      });
    },
    [candidateOptions, logPath, onOpenProfileReview, profileName, reviewProfile, reviewableInstallResult]
  );

  useEffect(() => {
    if (reviewableInstallResult === null || reviewProfile === null) {
      lastAutoOpenReviewKeyRef.current = null;
      return;
    }

    const autoOpenReviewKey = [
      reviewableInstallResult.profile_name.trim(),
      reviewableInstallResult.helper_log_path.trim(),
      reviewableInstallResult.message.trim(),
    ].join('::');

    if (lastAutoOpenReviewKeyRef.current === autoOpenReviewKey) {
      return;
    }

    lastAutoOpenReviewKeyRef.current = autoOpenReviewKey;
    openReviewPayload('install-complete');
  }, [openReviewPayload, reviewableInstallResult, reviewProfile]);

  const skipTrainerClass =
    launchMethod === 'native' ? ' crosshook-install-flow-tabs--skip-trainer' : '';

  return (
    <section className="crosshook-install-shell" aria-labelledby="install-game-heading">
      <div className="crosshook-install-shell__content">
        <div className="crosshook-install-intro">
          <div className="crosshook-heading-eyebrow">Install Game</div>
          <h3 id="install-game-heading" className="crosshook-heading-title crosshook-heading-title--install">
            Guided install shell
          </h3>
          <p className="crosshook-heading-copy">
            This guided flow runs the installer through Proton, surfaces a reviewable profile with full art, runtime,
            and preset support, and only persists the profile when you confirm Save.
          </p>
        </div>

        <Tabs.Root
          className={`crosshook-install-flow-tabs${skipTrainerClass}`}
          value={activeInstallTab}
          onValueChange={(value) => setActiveInstallTab(isInstallFlowTabId(value) ? value : 'identity')}
        >
          <Tabs.List className="crosshook-subtab-row" aria-label="Install flow sections">
            {installFlowTabs.map(({ id, label }) => (
              <Tabs.Trigger
                key={id}
                value={id}
                className={`crosshook-subtab${activeInstallTab === id ? ' crosshook-subtab--active' : ''}`}
              >
                {label}
              </Tabs.Trigger>
            ))}
          </Tabs.List>

          <Tabs.Content
            value="identity"
            forceMount
            className="crosshook-subtab-content"
            style={{ display: activeInstallTab === 'identity' ? undefined : 'none' }}
            aria-label={INSTALL_FLOW_TAB_LABELS.identity}
          >
            <div className="crosshook-subtab-content__inner crosshook-subtab-content__inner--wide-gap">
              <ProfileIdentitySection
                profileName={profileName}
                profile={draftProfile}
                onProfileNameChange={setProfileName}
                onUpdateProfile={updateDraftProfile}
              />
              <RunnerMethodSection profile={draftProfile} onUpdateProfile={updateDraftProfile} />
            </div>
          </Tabs.Content>

          <Tabs.Content
            value="runtime"
            forceMount
            className="crosshook-subtab-content"
            style={{ display: activeInstallTab === 'runtime' ? undefined : 'none' }}
            aria-label={INSTALL_FLOW_TAB_LABELS.runtime}
          >
            <div className="crosshook-subtab-content__inner crosshook-subtab-content__inner--wide-gap">
              <RuntimeSection
                profile={draftProfile}
                onUpdateProfile={updateDraftProfile}
                launchMethod={launchMethod}
                protonInstalls={protonInstalls}
                protonInstallsError={protonInstallsError}
              />
              <p className="crosshook-help-text">
                {prefixStateLabel(defaultPrefixPathState)}
                {defaultPrefixPath.trim().length > 0 ? ` Suggested default prefix: ${defaultPrefixPath}` : null}
                {defaultPrefixPathError ? ` ${defaultPrefixPathError}` : null}
              </p>
            </div>
          </Tabs.Content>

          {launchMethod !== 'native' ? (
            <Tabs.Content
              value="trainer"
              forceMount
              className="crosshook-subtab-content"
              style={{ display: activeInstallTab === 'trainer' ? undefined : 'none' }}
              aria-label={INSTALL_FLOW_TAB_LABELS.trainer}
            >
              <div className="crosshook-subtab-content__inner crosshook-subtab-content__inner--wide-gap">
                <TrainerSection
                  profile={draftProfile}
                  onUpdateProfile={updateDraftProfile}
                  launchMethod={launchMethod}
                  profileName={profileName}
                  profileExists={false}
                />
              </div>
            </Tabs.Content>
          ) : null}

          <Tabs.Content
            value="media"
            forceMount
            className="crosshook-subtab-content"
            style={{ display: activeInstallTab === 'media' ? undefined : 'none' }}
            aria-label={INSTALL_FLOW_TAB_LABELS.media}
          >
            <div className="crosshook-subtab-content__inner crosshook-subtab-content__inner--wide-gap">
              <MediaSection profile={draftProfile} onUpdateProfile={updateDraftProfile} launchMethod={launchMethod} />
            </div>
          </Tabs.Content>

          <Tabs.Content
            value="installer_review"
            forceMount
            className="crosshook-subtab-content"
            style={{ display: activeInstallTab === 'installer_review' ? undefined : 'none' }}
            aria-label={INSTALL_FLOW_TAB_LABELS.installer_review}
          >
            <div className="crosshook-subtab-content__inner crosshook-subtab-content__inner--wide-gap">
              <div className="crosshook-install-section">
                <div className="crosshook-install-section-title">Installer Media</div>
                <InstallField
                  label="Installer EXE"
                  value={installerInputs.installer_path}
                  onChange={(value) => updateInstallerInputs('installer_path', value)}
                  placeholder="/mnt/media/setup.exe"
                  browseLabel="Browse"
                  browseTitle="Select Installer Executable"
                  browseFilters={[{ name: 'Windows Executable', extensions: ['exe'] }]}
                  helpText="Choose the installer media, not the final game executable."
                  error={validation.fieldErrors.installer_path}
                />
              </div>

              <WizardPresetPicker
                bundledPresets={bundledOptimizationPresets}
                savedPresetNames={Object.keys(draftProfile.launch.presets ?? {})}
                activePresetKey={draftProfile.launch.active_preset ?? ''}
                busy={false}
                onApplyBundled={applyBundledPresetToDraft}
                onSelectSaved={applySavedPresetToDraft}
              />

              <CustomEnvironmentVariablesSection
                profileName={profileName}
                customEnvVars={draftProfile.launch.custom_env_vars ?? {}}
                onUpdateProfile={updateDraftProfile}
                idPrefix="install-env"
              />

              <InstallReviewSummary
                installation={{
                  stage,
                  statusText,
                  hintText,
                  error,
                  generalError: validation.generalError,
                  candidateOptions,
                  currentExecutablePath: draftProfile.game.executable_path,
                  onSelectCandidate: setInstalledExecutablePath,
                  onFinalExecutableChange: setInstalledExecutablePath,
                  finalExecutableError: validation.fieldErrors.installed_game_executable_path,
                  helperLogPath: logPath,
                  isRunningInstaller,
                  defaultPrefixPathState,
                  candidateCount: candidateOptions.length,
                }}
                validation={installValidation}
              />
            </div>
          </Tabs.Content>
        </Tabs.Root>
      </div>

      <div className="crosshook-install-shell__footer crosshook-route-footer">
        <div className="crosshook-install-shell__actions">
          <button
            type="button"
            className="crosshook-button"
            disabled={
              isRunningInstaller || isResolvingDefaultPrefixPath || !installValidation.isReady
            }
            aria-describedby={!installValidation.isReady ? installRequiredHintId : undefined}
            onClick={async () => {
              const shouldProceed = await Promise.resolve(onRequestInstallAction?.('retry') ?? true);
              if (!shouldProceed) {
                return;
              }

              await startInstall();
            }}
          >
            {actionLabel}
          </button>
          <button
            type="button"
            className="crosshook-button crosshook-button--secondary"
            onClick={async () => {
              const shouldProceed = await Promise.resolve(onRequestInstallAction?.('reset') ?? true);
              if (!shouldProceed) {
                return;
              }

              reset();
            }}
          >
            Reset Form
          </button>
          <div className="crosshook-help-text crosshook-install-shell__actions-guidance">
            {isResolvingDefaultPrefixPath
              ? 'Resolving the suggested prefix path before install.'
              : 'The generated profile stays editable until the modal save step.'}
          </div>
          {reviewableInstallResult !== null ? (
            <button
              type="button"
              className="crosshook-button crosshook-button--secondary"
              disabled={!canReviewGeneratedProfile}
              onClick={() => openReviewPayload('manual-verify')}
            >
              Open Review Modal
            </button>
          ) : null}
          {!installValidation.isReady ? (
            <span id={installRequiredHintId} className="crosshook-help-text">
              {installValidation.firstMissingId
                ? `Complete required fields (first missing: ${installValidation.firstMissingId}).`
                : 'Complete required fields before installing.'}
            </span>
          ) : null}
        </div>
      </div>
    </section>
  );
}

export default InstallGamePanel;
