import { useEffect, useId, useMemo, useState, type ChangeEvent, type ReactNode } from 'react';

import { CustomEnvironmentVariablesSection } from './CustomEnvironmentVariablesSection';
import ProtonDbOverwriteConfirmation from './ProtonDbOverwriteConfirmation';
import ProtonDbLookupCard from './ProtonDbLookupCard';
import { ThemedSelect } from './ui/ThemedSelect';
import { ProfileIdentitySection } from './profile-sections/ProfileIdentitySection';
import { GameSection } from './profile-sections/GameSection';
import { RunnerMethodSection } from './profile-sections/RunnerMethodSection';
import { TrainerSection } from './profile-sections/TrainerSection';
import { RuntimeSection } from './profile-sections/RuntimeSection';
import { useSetTrainerVersion } from '../hooks/useSetTrainerVersion';
import type { GameProfile, LaunchMethod } from '../types';
import type { ProtonInstallOption } from '../types/proton';
import type { ProtonDbRecommendationGroup } from '../types/protondb';
import type { VersionCorrelationStatus } from '../types/version';
import { mergeProtonDbEnvVarGroup, type PendingProtonDbOverwrite } from '../utils/protondb';
import { formatProtonInstallLabel } from '../utils/proton';

export type ProfileFormSectionsProfileSelector = {
  profiles: string[];
  favoriteProfiles: string[];
  selectedProfile: string;
  onSelectProfile: (name: string) => Promise<void>;
  onToggleFavorite: (name: string, favorite: boolean) => Promise<void>;
};

type ProfileFormSectionsBaseProps = {
  profileName: string;
  profile: GameProfile;
  launchMethod: LaunchMethod;
  protonInstalls: ProtonInstallOption[];
  protonInstallsError: string | null;
  reviewMode?: boolean;
  profileExists?: boolean;
  trainerVersion?: string | null;
  versionStatus?: VersionCorrelationStatus | null;
  onVersionSet?: () => void;
  onProfileNameChange: (value: string) => void;
  onUpdateProfile: (updater: (current: GameProfile) => GameProfile) => void;
};

export type ProfileFormSectionsProps =
  | (ProfileFormSectionsBaseProps & { profileSelector: ProfileFormSectionsProfileSelector })
  | (ProfileFormSectionsBaseProps & { profileSelector?: undefined });

const launcherNameHelperText =
  'CrossHook appends " - Trainer" to the exported launcher title. Enter only the base launcher name here.';

const optionalSectionStyle = {
  display: 'grid',
  gap: 12,
  padding: 14,
  borderRadius: 14,
  border: '1px solid rgba(255, 255, 255, 0.08)',
  background: 'rgba(255, 255, 255, 0.03)',
};

const optionalSectionSummaryStyle = {
  cursor: 'pointer',
  color: 'var(--crosshook-color-text-muted)',
  fontWeight: 600,
  listStyle: 'none',
  outline: 'none',
};

export type { PendingProtonDbOverwrite };

export function parentDirectory(path: string): string {
  const normalized = path.trim().replace(/\\/g, '/');
  const separatorIndex = normalized.lastIndexOf('/');

  if (separatorIndex <= 0) {
    return '';
  }

  return normalized.slice(0, separatorIndex);
}

export function updateGameExecutablePath(current: GameProfile, nextExecutablePath: string): GameProfile {
  const previousExecutableParent = parentDirectory(current.game.executable_path);
  const currentWorkingDirectory = current.runtime.working_directory.trim();
  const shouldDeriveWorkingDirectory =
    currentWorkingDirectory.length === 0 || currentWorkingDirectory === previousExecutableParent;

  return {
    ...current,
    game: {
      ...current.game,
      executable_path: nextExecutablePath,
    },
    runtime: {
      ...current.runtime,
      working_directory: shouldDeriveWorkingDirectory
        ? parentDirectory(nextExecutablePath)
        : current.runtime.working_directory,
    },
  };
}

export { deriveSteamClientInstallPath } from '../utils/steam';
export { formatProtonInstallLabel } from '../utils/proton';
export type { ProtonInstallOption } from '../types/proton';

export function FieldRow(props: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  helperText?: string;
  error?: string | null;
  browseLabel?: string;
  onBrowse?: () => Promise<void>;
}) {
  const inputId = useId();

  return (
    <div className="crosshook-field">
      <label className="crosshook-label" htmlFor={inputId}>
        {props.label}
      </label>
      <div className="crosshook-install-field-control">
        <input
          id={inputId}
          className="crosshook-input"
          style={{ flex: 1, minWidth: 0 }}
          value={props.value}
          placeholder={props.placeholder}
          onChange={(event: ChangeEvent<HTMLInputElement>) => props.onChange(event.target.value)}
        />
        {props.onBrowse ? (
          <button
            type="button"
            className="crosshook-button crosshook-button--secondary"
            onClick={() => void props.onBrowse?.()}
          >
            {props.browseLabel ?? 'Browse'}
          </button>
        ) : null}
      </div>
      {props.helperText ? <p className="crosshook-help-text">{props.helperText}</p> : null}
      {props.error ? <p className="crosshook-danger">{props.error}</p> : null}
    </div>
  );
}

export function ProtonPathField(props: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder: string;
  installs: ProtonInstallOption[];
  error: string | null;
  installsError: string | null;
  onBrowse: () => Promise<void>;
}) {
  const duplicateNameCounts = props.installs.reduce<Record<string, number>>((counts, install) => {
    const key = install.name.trim() || 'Unnamed Proton install';
    counts[key] = (counts[key] ?? 0) + 1;
    return counts;
  }, {});
  const selectId = useId();
  const inputId = useId();
  const selectedInstallPath = props.installs.find((install) => install.path.trim() === props.value.trim())?.path ?? '';

  return (
    <div className="crosshook-field crosshook-install-proton-field">
      <label className="crosshook-label" htmlFor={selectId}>
        {props.label}
      </label>
      <div style={{ display: 'grid', gap: 10 }}>
        <ThemedSelect
          id={selectId}
          value={selectedInstallPath}
          onValueChange={(val) => {
            if (val.trim().length > 0) {
              props.onChange(val);
            }
          }}
          placeholder="Detected Proton install"
          options={props.installs.map((install) => ({
            value: install.path,
            label: formatProtonInstallLabel(install, duplicateNameCounts),
          }))}
        />

        <div className="crosshook-install-field-control">
          <input
            id={inputId}
            className="crosshook-input"
            style={{ flex: 1, minWidth: 0 }}
            value={props.value}
            onChange={(event: ChangeEvent<HTMLInputElement>) => props.onChange(event.target.value)}
            placeholder={props.placeholder}
          />
          <button
            type="button"
            className="crosshook-button crosshook-button--secondary"
            onClick={() => void props.onBrowse()}
          >
            Browse
          </button>
        </div>
      </div>

      <p className="crosshook-help-text">
        Pick a detected Proton install to fill this field automatically, or edit the path manually.
      </p>
      {props.error ? <p className="crosshook-danger">{props.error}</p> : null}
      {props.installsError ? <p className="crosshook-danger">{props.installsError}</p> : null}
    </div>
  );
}

export function LauncherMetadataFields(props: {
  profile: GameProfile;
  onUpdateProfile: (updater: (current: GameProfile) => GameProfile) => void;
}) {
  return (
    <FieldRow
      label="Launcher Name"
      value={props.profile.steam.launcher.display_name}
      onChange={(value) =>
        props.onUpdateProfile((current) => ({
          ...current,
          steam: {
            ...current.steam,
            launcher: { ...current.steam.launcher, display_name: value },
          },
        }))
      }
      placeholder="God of War Ragnarok"
      helperText={launcherNameHelperText}
    />
  );
}

export function OptionalSection(props: { summary: string; children: ReactNode; collapsed: boolean }) {
  if (!props.collapsed) {
    return <>{props.children}</>;
  }

  return (
    <details style={optionalSectionStyle}>
      <summary style={optionalSectionSummaryStyle}>{props.summary}</summary>
      <div style={{ marginTop: 4 }}>{props.children}</div>
    </details>
  );
}

function ProfileSelectorField({
  profileNamesListId,
  profileSelector,
  selectedProfile,
}: {
  profileNamesListId: string;
  profileSelector: ProfileFormSectionsProfileSelector;
  selectedProfile: string;
}) {
  const isPinned = selectedProfile !== '' && profileSelector.favoriteProfiles.includes(selectedProfile);
  const pinnedSet = useMemo(() => new Set(profileSelector.favoriteProfiles), [profileSelector.favoriteProfiles]);
  const handleTogglePin = useMemo(
    () => (value: string) => {
      void profileSelector.onToggleFavorite(value, !pinnedSet.has(value));
    },
    [pinnedSet, profileSelector]
  );

  return (
    <div className="crosshook-field">
      <label className="crosshook-label" htmlFor={`${profileNamesListId}-selector`}>
        Load Profile
      </label>
      <div style={{ display: 'flex', gap: 8, alignItems: 'stretch' }}>
        <div style={{ flex: '1 1 0', minWidth: 0 }}>
          <ThemedSelect
            id={`${profileNamesListId}-selector`}
            value={selectedProfile}
            onValueChange={(val) => void profileSelector.onSelectProfile(val)}
            placeholder="Create New"
            pinnedValues={pinnedSet}
            onTogglePin={handleTogglePin}
            options={[
              { value: '', label: 'Create New' },
              ...profileSelector.profiles.map((name) => ({ value: name, label: name })),
            ]}
          />
        </div>
        {selectedProfile !== '' ? (
          <button
            type="button"
            className={`crosshook-profile-pin-btn${isPinned ? ' crosshook-profile-pin-btn--active' : ''}`}
            onClick={() => void profileSelector.onToggleFavorite(selectedProfile, !isPinned)}
            aria-label={isPinned ? `Unpin ${selectedProfile}` : `Pin ${selectedProfile}`}
            title={isPinned ? 'Remove from pinned' : 'Pin to top'}
          >
            {isPinned ? '\u2605' : '\u2606'}
          </button>
        ) : null}
      </div>
    </div>
  );
}

export function TrainerVersionSetField({ profileName, onVersionSet }: { profileName: string; onVersionSet?: () => void }) {
  const [pendingVersion, setPendingVersion] = useState('');
  const inputId = useId();
  const { setting, error, success, setVersion, clearSuccess } = useSetTrainerVersion(profileName, onVersionSet);

  const handleSet = async () => {
    const saved = await setVersion(pendingVersion);
    if (saved) {
      setPendingVersion('');
    }
  };

  return (
    <div className="crosshook-field">
      <label className="crosshook-label" htmlFor={inputId}>
        Set Trainer Version
      </label>
      <div className="crosshook-install-field-control">
        <input
          id={inputId}
          className="crosshook-input"
          style={{ flex: 1, minWidth: 0 }}
          value={pendingVersion}
          placeholder="e.g. v1.0.2 or 2024.01.15"
          onChange={(event: ChangeEvent<HTMLInputElement>) => {
            setPendingVersion(event.target.value);
            clearSuccess();
          }}
          onKeyDown={(event) => {
            if (event.key === 'Enter') void handleSet();
          }}
        />
        <button
          type="button"
          className="crosshook-button crosshook-button--secondary"
          onClick={() => void handleSet()}
          disabled={setting || !pendingVersion.trim()}
        >
          {setting ? 'Saving...' : 'Set'}
        </button>
      </div>
      <p className="crosshook-help-text">Manually record the trainer version when it cannot be auto-detected.</p>
      {error ? <p className="crosshook-danger">{error}</p> : null}
      {success ? (
        <p className="crosshook-help-text" role="status">
          Trainer version saved.
        </p>
      ) : null}
    </div>
  );
}

export function ProfileFormSections(props: ProfileFormSectionsProps) {
  const {
    profileName,
    profile,
    launchMethod,
    protonInstalls,
    protonInstallsError,
    reviewMode = false,
    profileExists = false,
    trainerVersion = null,
    versionStatus = null,
    onVersionSet,
    onProfileNameChange,
    onUpdateProfile,
  } = props;
  const profileSelector = 'profileSelector' in props ? props.profileSelector : undefined;
  const profileNamesListId = useId();
  const [pendingProtonDbOverwrite, setPendingProtonDbOverwrite] = useState<PendingProtonDbOverwrite | null>(null);
  const [applyingProtonDbGroupId, setApplyingProtonDbGroupId] = useState<string | null>(null);
  const [protonDbStatusMessage, setProtonDbStatusMessage] = useState<string | null>(null);

  const profiles = profileSelector?.profiles;
  const selectedProfile = profileSelector?.selectedProfile;
  const showProtonDbLookup = launchMethod === 'steam_applaunch' || launchMethod === 'proton_run';
  const reviewModeNote = reviewMode ? (
    <p className="crosshook-help-text">
      Review mode keeps launch-critical fields expanded and collapses only empty optional overrides.
    </p>
  ) : null;

  useEffect(() => {
    setPendingProtonDbOverwrite(null);
    setApplyingProtonDbGroupId(null);
    setProtonDbStatusMessage(null);
  }, [profileName, profile.steam.app_id, launchMethod]);

  const applyProtonDbGroup = (group: ProtonDbRecommendationGroup, overwriteKeys: readonly string[]) => {
    const merge = {
      appliedKeys: [] as string[],
      unchangedKeys: [] as string[],
    };
    onUpdateProfile((current) => {
      const nextMerge = mergeProtonDbEnvVarGroup(current.launch.custom_env_vars, group, overwriteKeys);
      merge.appliedKeys = nextMerge.appliedKeys;
      merge.unchangedKeys = nextMerge.unchangedKeys;
      return {
        ...current,
        launch: {
          ...current.launch,
          custom_env_vars: nextMerge.mergedEnvVars,
        },
      };
    });
    setApplyingProtonDbGroupId(null);
    setPendingProtonDbOverwrite(null);

    const appliedCount = merge.appliedKeys.length;
    const unchangedCount = merge.unchangedKeys.length;
    if (appliedCount > 0) {
      setProtonDbStatusMessage(
        `Applied ${appliedCount} ProtonDB environment variable${appliedCount === 1 ? '' : 's'}${
          unchangedCount > 0
            ? ` and left ${unchangedCount} existing match${unchangedCount === 1 ? '' : 'es'} unchanged`
            : ''
        }.`
      );
      return;
    }

    if (unchangedCount > 0) {
      setProtonDbStatusMessage('All suggested ProtonDB environment variables already match the current profile.');
      return;
    }

    setProtonDbStatusMessage('No ProtonDB environment-variable changes were applied.');
  };

  const handleApplyProtonDbEnvVars = (group: ProtonDbRecommendationGroup) => {
    const envVars = group.env_vars ?? [];
    if (envVars.length === 0) {
      return;
    }

    setApplyingProtonDbGroupId(group.group_id?.trim() || group.title?.trim() || null);
    const merge = mergeProtonDbEnvVarGroup(profile.launch.custom_env_vars, group);
    if (merge.conflicts.length === 0) {
      applyProtonDbGroup(group, []);
      return;
    }

    setApplyingProtonDbGroupId(null);
    setPendingProtonDbOverwrite({
      group,
      conflicts: merge.conflicts,
      resolutions: Object.fromEntries(merge.conflicts.map((conflict) => [conflict.key, 'keep_current' as const])),
    });
    setProtonDbStatusMessage(null);
  };

  const protonDbPanel = showProtonDbLookup ? (
    <div className="crosshook-protondb-panel">
      <ProtonDbLookupCard
        appId={profile.steam.app_id}
        trainerVersion={trainerVersion}
        versionContext={{ version_status: versionStatus }}
        onApplyEnvVars={reviewMode ? undefined : handleApplyProtonDbEnvVars}
        applyingGroupId={applyingProtonDbGroupId}
      />

      {protonDbStatusMessage ? (
        <p className="crosshook-help-text" role="status">
          {protonDbStatusMessage}
        </p>
      ) : null}

      {pendingProtonDbOverwrite ? (
        <ProtonDbOverwriteConfirmation
          pendingProtonDbOverwrite={pendingProtonDbOverwrite}
          onUpdateProtonDbResolution={(key, choice) =>
            setPendingProtonDbOverwrite((current) =>
              current == null
                ? current
                : {
                    ...current,
                    resolutions: {
                      ...current.resolutions,
                      [key]: choice,
                    },
                  }
            )
          }
          onCancelProtonDbOverwrite={() => setPendingProtonDbOverwrite(null)}
          onConfirmProtonDbOverwrite={(selectedKeys) =>
            applyProtonDbGroup(pendingProtonDbOverwrite.group, selectedKeys)
          }
        />
      ) : null}
    </div>
  ) : null;

  return (
    <div className="crosshook-profile-shell">
      {reviewModeNote}

      <ProfileIdentitySection
        profileName={profileName}
        profile={profile}
        onProfileNameChange={onProfileNameChange}
        onUpdateProfile={onUpdateProfile}
        reviewMode={reviewMode}
        profileExists={profileExists}
        profiles={profiles}
      />

      {profileSelector ? (
        <ProfileSelectorField
          profileNamesListId={profileNamesListId}
          profileSelector={profileSelector}
          selectedProfile={selectedProfile ?? ''}
        />
      ) : null}

      <GameSection
        profile={profile}
        onUpdateProfile={onUpdateProfile}
        reviewMode={reviewMode}
        launchMethod={launchMethod}
      />

      <RunnerMethodSection
        profile={profile}
        onUpdateProfile={onUpdateProfile}
        reviewMode={reviewMode}
      />

      <CustomEnvironmentVariablesSection
        profileName={profileName}
        customEnvVars={profile.launch.custom_env_vars}
        onUpdateProfile={onUpdateProfile}
        idPrefix={profileNamesListId}
      />

      <TrainerSection
        profile={profile}
        onUpdateProfile={onUpdateProfile}
        reviewMode={reviewMode}
        launchMethod={launchMethod}
        profileName={profileName}
        profileExists={profileExists}
        trainerVersion={trainerVersion}
        onVersionSet={onVersionSet}
      />

      <RuntimeSection
        profile={profile}
        onUpdateProfile={onUpdateProfile}
        reviewMode={reviewMode}
        launchMethod={launchMethod}
        protonInstalls={protonInstalls}
        protonInstallsError={protonInstallsError}
        protonDbPanel={protonDbPanel}
      />
    </div>
  );
}

export default ProfileFormSections;
