import { useId, type ChangeEvent, type ReactNode } from 'react';

import AutoPopulate from './AutoPopulate';
import { chooseDirectory, chooseFile } from '../utils/dialog';
import type { GameProfile, LaunchMethod } from '../types';

export interface ProtonInstallOption {
  name: string;
  path: string;
  is_official: boolean;
}

export type ProfileFormSectionsProfileSelector = {
  profiles: string[];
  selectedProfile: string;
  onSelectProfile: (name: string) => Promise<void>;
};

type ProfileFormSectionsBaseProps = {
  profileName: string;
  profile: GameProfile;
  launchMethod: LaunchMethod;
  protonInstalls: ProtonInstallOption[];
  protonInstallsError: string | null;
  reviewMode?: boolean;
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

function parentDirectory(path: string): string {
  const normalized = path.trim().replace(/\\/g, '/');
  const separatorIndex = normalized.lastIndexOf('/');

  if (separatorIndex <= 0) {
    return '';
  }

  return normalized.slice(0, separatorIndex);
}

function updateGameExecutablePath(current: GameProfile, nextExecutablePath: string): GameProfile {
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
      working_directory: shouldDeriveWorkingDirectory ? parentDirectory(nextExecutablePath) : current.runtime.working_directory,
    },
  };
}

export function deriveSteamClientInstallPath(compatdataPath: string): string {
  const marker = '/steamapps/compatdata/';
  const normalized = compatdataPath.trim().replace(/\\/g, '/');
  const index = normalized.indexOf(marker);

  return index >= 0 ? normalized.slice(0, index) : '';
}

export function formatProtonInstallLabel(install: ProtonInstallOption, duplicateNameCounts: Record<string, number>): string {
  const baseLabel = install.name.trim() || 'Unnamed Proton install';
  if ((duplicateNameCounts[baseLabel] ?? 0) <= 1) {
    return baseLabel;
  }

  return `${baseLabel} (${install.is_official ? 'Steam' : 'Custom'})`;
}

function FieldRow(props: {
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

function ProtonPathField(props: {
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
        <select
          id={selectId}
          className="crosshook-select"
          value={selectedInstallPath}
          onChange={(event) => {
            if (event.target.value.trim().length > 0) {
              props.onChange(event.target.value);
            }
          }}
        >
          <option value="">Detected Proton install</option>
          {props.installs.map((install) => (
            <option key={install.path} value={install.path}>
              {formatProtonInstallLabel(install, duplicateNameCounts)}
            </option>
          ))}
        </select>

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

function LauncherMetadataFields(props: {
  profile: GameProfile;
  onUpdateProfile: (updater: (current: GameProfile) => GameProfile) => void;
}) {
  return (
    <>
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

      <FieldRow
        label="Launcher Icon"
        value={props.profile.steam.launcher.icon_path}
        onChange={(value) =>
          props.onUpdateProfile((current) => ({
            ...current,
            steam: {
              ...current.steam,
              launcher: { ...current.steam.launcher, icon_path: value },
            },
          }))
        }
        placeholder="/path/to/icon.png"
        browseLabel="Browse"
        onBrowse={async () => {
          const path = await chooseFile('Select Launcher Icon', [
            { name: 'Images', extensions: ['png', 'jpg', 'jpeg'] },
          ]);

          if (path) {
            props.onUpdateProfile((current) => ({
              ...current,
              steam: {
                ...current.steam,
                launcher: { ...current.steam.launcher, icon_path: path },
              },
            }));
          }
        }}
      />
    </>
  );
}

function OptionalSection(props: {
  summary: string;
  children: ReactNode;
  collapsed: boolean;
}) {
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

export function ProfileFormSections(props: ProfileFormSectionsProps) {
  const {
    profileName,
    profile,
    launchMethod,
    protonInstalls,
    protonInstallsError,
    reviewMode = false,
    onProfileNameChange,
    onUpdateProfile,
  } = props;
  const profileSelector = 'profileSelector' in props ? props.profileSelector : undefined;
  const profileNamesListId = useId();
  const showProfileSelector = profileSelector !== undefined;
  const profiles = profileSelector?.profiles;
  const selectedProfile = profileSelector?.selectedProfile;
  const supportsTrainerLaunch = launchMethod !== 'native';
  const steamClientInstallPath = deriveSteamClientInstallPath(profile.steam.compatdata_path);
  const trainerCollapsed = reviewMode && profile.trainer.path.trim().length === 0;
  const workingDirectoryCollapsed = reviewMode && profile.runtime.working_directory.trim().length === 0;
  const showLauncherMetadata = supportsTrainerLaunch && !reviewMode;
  const reviewModeNote = reviewMode ? (
    <p className="crosshook-help-text">
      Review mode keeps launch-critical fields expanded and collapses only empty optional overrides.
    </p>
  ) : null;

  return (
    <div className="crosshook-profile-shell">
      {reviewModeNote}

      <div className="crosshook-install-section-title">Profile Identity</div>
      <div
        style={{
          display: 'grid',
          gap: 12,
          gridTemplateColumns: showProfileSelector ? 'minmax(0, 1fr) auto' : 'minmax(0, 1fr)',
        }}
      >
        <div className="crosshook-field">
          <label className="crosshook-label" htmlFor={profileNamesListId}>
            Profile Name
          </label>
          <input
            id={profileNamesListId}
            className="crosshook-input"
            list={profiles && profiles.length > 0 ? `${profileNamesListId}-suggestions` : undefined}
            value={profileName}
            placeholder="Enter or choose a profile name"
            onChange={(event: ChangeEvent<HTMLInputElement>) => onProfileNameChange(event.target.value)}
          />
          {profiles && profiles.length > 0 ? (
            <datalist id={`${profileNamesListId}-suggestions`}>
              {profiles.map((name) => (
                <option key={name} value={name} />
              ))}
            </datalist>
          ) : null}
        </div>

        {profileSelector ? (
          <div className="crosshook-field">
            <label className="crosshook-label" htmlFor={`${profileNamesListId}-selector`}>
              Load Profile
            </label>
            <select
              id={`${profileNamesListId}-selector`}
              className="crosshook-select"
              value={selectedProfile}
              onChange={(event) => void profileSelector.onSelectProfile(event.target.value)}
            >
              <option value="">Create New</option>
              {profileSelector.profiles.map((name) => (
                <option key={name} value={name}>
                  {name}
                </option>
              ))}
            </select>
          </div>
        ) : null}
      </div>

      <div className="crosshook-install-section-title">Game</div>
      <div className="crosshook-install-grid">
        <FieldRow
          label="Game Name"
          value={profile.game.name}
          onChange={(value) =>
            onUpdateProfile((current) => ({
              ...current,
              game: { ...current.game, name: value },
            }))
          }
          placeholder="God of War Ragnarok"
        />

        <FieldRow
          label="Game Path"
          value={profile.game.executable_path}
          onChange={(value) =>
            onUpdateProfile((current) => updateGameExecutablePath(current, value))
          }
          placeholder="/path/to/game.exe"
          browseLabel="Browse"
          onBrowse={async () => {
            const path =
              launchMethod === 'native'
                ? await chooseFile('Select Linux Game Executable')
                : await chooseFile('Select Game Executable', [{ name: 'Windows Executable', extensions: ['exe'] }]);

            if (path) {
              onUpdateProfile((current) => updateGameExecutablePath(current, path));
            }
          }}
        />
      </div>

      <div className="crosshook-install-section-title">Runner Method</div>
      <div className="crosshook-field">
        <label className="crosshook-label" htmlFor={`${profileNamesListId}-launch-method`}>
          Runner Method
        </label>
        <select
          id={`${profileNamesListId}-launch-method`}
          className="crosshook-select"
          value={launchMethod}
          onChange={(event) =>
            onUpdateProfile((current) => ({
              ...current,
              steam: { ...current.steam, enabled: event.target.value === 'steam_applaunch' },
              launch: {
                ...current.launch,
                method: event.target.value as typeof current.launch.method,
              },
            }))
          }
        >
          <option value="steam_applaunch">Steam app launch</option>
          <option value="proton_run">Proton runtime launch</option>
          <option value="native">Native Linux launch</option>
        </select>
        <p className="crosshook-help-text">
          Choose the runner explicitly so CrossHook saves the correct launch method and only shows the relevant fields.
        </p>
      </div>

      {supportsTrainerLaunch ? (
        <div className="crosshook-install-section">
          <div className="crosshook-install-section-title">Trainer</div>
          <OptionalSection summary="Trainer details" collapsed={trainerCollapsed}>
            <div className="crosshook-install-grid">
              <FieldRow
                label="Trainer Path"
                value={profile.trainer.path}
                onChange={(value) =>
                  onUpdateProfile((current) => ({
                    ...current,
                    trainer: { ...current.trainer, path: value },
                  }))
                }
                placeholder="/path/to/trainer.exe"
                browseLabel="Browse"
                onBrowse={async () => {
                  const path = await chooseFile('Select Trainer Executable', [
                    { name: 'Windows Executable', extensions: ['exe'] },
                  ]);

                  if (path) {
                    onUpdateProfile((current) => ({
                      ...current,
                      trainer: { ...current.trainer, path },
                    }));
                  }
                }}
              />
            </div>
          </OptionalSection>
        </div>
      ) : null}

      <div className="crosshook-install-section">
        <div className="crosshook-install-section-title">
          {launchMethod === 'steam_applaunch'
            ? 'Steam Runtime'
            : launchMethod === 'proton_run'
              ? 'Proton Runtime'
              : 'Native Runtime'}
        </div>
        {launchMethod === 'steam_applaunch' ? (
          <>
            <div className="crosshook-install-grid">
              <FieldRow
                label="Steam App ID"
                value={profile.steam.app_id}
                onChange={(value) =>
                  onUpdateProfile((current) => ({
                    ...current,
                    steam: { ...current.steam, app_id: value },
                  }))
                }
                placeholder="1245620"
              />

              <FieldRow
                label="Prefix Path"
                value={profile.steam.compatdata_path}
                onChange={(value) =>
                  onUpdateProfile((current) => ({
                    ...current,
                    steam: { ...current.steam, compatdata_path: value },
                  }))
                }
                placeholder="/home/user/.local/share/Steam/steamapps/compatdata/1245620"
                browseLabel="Browse"
                onBrowse={async () => {
                  const path = await chooseDirectory('Select Steam Prefix Directory');

                  if (path) {
                    onUpdateProfile((current) => ({
                      ...current,
                      steam: { ...current.steam, compatdata_path: path },
                    }));
                  }
                }}
              />

              {showLauncherMetadata ? (
                <LauncherMetadataFields
                  profile={profile}
                  onUpdateProfile={onUpdateProfile}
                />
              ) : null}
            </div>

            <ProtonPathField
              label="Proton Path"
              value={profile.steam.proton_path}
              onChange={(value) =>
                onUpdateProfile((current) => ({
                  ...current,
                  steam: { ...current.steam, proton_path: value },
                }))
              }
              placeholder="/home/user/.steam/root/steamapps/common/Proton - Experimental/proton"
              installs={protonInstalls}
              error={null}
              installsError={protonInstallsError}
              onBrowse={async () => {
                const path = await chooseFile('Select Proton Executable');

                if (path) {
                  onUpdateProfile((current) => ({
                    ...current,
                    steam: { ...current.steam, proton_path: path },
                  }));
                }
              }}
            />

            <div style={{ display: 'grid', gap: 16, marginTop: 18 }}>
              <AutoPopulate
                gamePath={profile.game.executable_path}
                steamClientInstallPath={steamClientInstallPath}
                currentAppId={profile.steam.app_id}
                currentCompatdataPath={profile.steam.compatdata_path}
                currentProtonPath={profile.steam.proton_path}
                onApplyAppId={(value) =>
                  onUpdateProfile((current) => ({
                    ...current,
                    steam: { ...current.steam, app_id: value },
                  }))
                }
                onApplyCompatdataPath={(value) =>
                  onUpdateProfile((current) => ({
                    ...current,
                    steam: { ...current.steam, compatdata_path: value },
                  }))
                }
                onApplyProtonPath={(value) =>
                  onUpdateProfile((current) => ({
                    ...current,
                    steam: { ...current.steam, proton_path: value },
                  }))
                }
              />
            </div>
          </>
        ) : null}

        {launchMethod === 'proton_run' ? (
          <>
            <div className="crosshook-install-grid">
              <FieldRow
                label="Prefix Path"
                value={profile.runtime.prefix_path}
                onChange={(value) =>
                  onUpdateProfile((current) => ({
                    ...current,
                    runtime: { ...current.runtime, prefix_path: value },
                  }))
                }
                placeholder="/path/to/prefix"
                browseLabel="Browse"
                onBrowse={async () => {
                  const path = await chooseDirectory('Select Proton Prefix Directory');

                  if (path) {
                    onUpdateProfile((current) => ({
                      ...current,
                      runtime: { ...current.runtime, prefix_path: path },
                    }));
                  }
                }}
              />

              {showLauncherMetadata ? (
                <LauncherMetadataFields
                  profile={profile}
                  onUpdateProfile={onUpdateProfile}
                />
              ) : null}

              <OptionalSection summary="Working directory override" collapsed={workingDirectoryCollapsed}>
                <FieldRow
                  label="Working Directory"
                  value={profile.runtime.working_directory}
                  onChange={(value) =>
                    onUpdateProfile((current) => ({
                      ...current,
                      runtime: { ...current.runtime, working_directory: value },
                    }))
                  }
                  placeholder="Optional override"
                  browseLabel="Browse"
                  onBrowse={async () => {
                    const path = await chooseDirectory('Select Working Directory');

                    if (path) {
                      onUpdateProfile((current) => ({
                        ...current,
                        runtime: { ...current.runtime, working_directory: path },
                      }));
                    }
                  }}
                />
              </OptionalSection>
            </div>

            <ProtonPathField
              label="Proton Path"
              value={profile.runtime.proton_path}
              onChange={(value) =>
                onUpdateProfile((current) => ({
                  ...current,
                  runtime: { ...current.runtime, proton_path: value },
                }))
              }
              placeholder="/path/to/proton"
              installs={protonInstalls}
              error={null}
              installsError={protonInstallsError}
              onBrowse={async () => {
                const path = await chooseFile('Select Proton Executable');

                if (path) {
                  onUpdateProfile((current) => ({
                    ...current,
                    runtime: { ...current.runtime, proton_path: path },
                  }));
                }
              }}
            />
          </>
        ) : null}

        {launchMethod === 'native' ? (
          <OptionalSection summary="Working directory override" collapsed={workingDirectoryCollapsed}>
            <div className="crosshook-install-grid" style={{ marginTop: 16 }}>
              <FieldRow
                label="Working Directory"
                value={profile.runtime.working_directory}
                onChange={(value) =>
                  onUpdateProfile((current) => ({
                    ...current,
                    runtime: { ...current.runtime, working_directory: value },
                  }))
                }
                placeholder="Optional override"
                browseLabel="Browse"
                onBrowse={async () => {
                  const path = await chooseDirectory('Select Working Directory');

                  if (path) {
                    onUpdateProfile((current) => ({
                      ...current,
                      runtime: { ...current.runtime, working_directory: path },
                    }));
                  }
                }}
              />
            </div>
          </OptionalSection>
        ) : null}
      </div>
    </div>
  );
}

export default ProfileFormSections;
