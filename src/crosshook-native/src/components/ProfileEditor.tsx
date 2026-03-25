import { useEffect, useState, type CSSProperties, type ChangeEvent } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import AutoPopulate from './AutoPopulate';
import InstallGamePanel from './InstallGamePanel';
import { useProfile, type UseProfileResult } from '../hooks/useProfile';
import type { GameProfile } from '../types';

const panelStyle: CSSProperties = {
  background: 'rgba(13, 20, 31, 0.92)',
  border: '1px solid rgba(120, 145, 177, 0.2)',
  borderRadius: 18,
  boxShadow: '0 24px 60px rgba(0, 0, 0, 0.35)',
  padding: 20,
};

const fieldStyle: CSSProperties = {
  display: 'grid',
  gap: 8,
};

const inputStyle: CSSProperties = {
  width: '100%',
  minWidth: 0,
  minHeight: 44,
  borderRadius: 12,
  border: '1px solid rgba(120, 145, 177, 0.35)',
  background: '#08111c',
  color: '#f3f6fb',
  padding: '0 14px',
  boxSizing: 'border-box',
};

const labelStyle: CSSProperties = {
  fontSize: 13,
  fontWeight: 600,
  color: '#b8c4d7',
  letterSpacing: '0.02em',
};

const buttonStyle: CSSProperties = {
  minHeight: 42,
  borderRadius: 12,
  border: '1px solid rgba(120, 145, 177, 0.35)',
  background: 'linear-gradient(180deg, #1a2b45 0%, #132034 100%)',
  color: '#f3f6fb',
  padding: '0 14px',
  cursor: 'pointer',
};

const subtleButtonStyle: CSSProperties = {
  ...buttonStyle,
  background: '#0b1624',
};

const helperStyle: CSSProperties = {
  margin: 0,
  color: '#99a8bd',
  fontSize: 13,
  lineHeight: 1.5,
};

const launcherNameHelperText =
  'CrossHook appends " - Trainer" to the exported launcher title. Enter only the base launcher name here.';

function FieldRow(props: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  helperText?: string;
  browseLabel?: string;
  onBrowse?: () => Promise<void>;
}) {
  return (
    <div style={fieldStyle}>
      <label style={labelStyle}>{props.label}</label>
      <div style={{ display: 'flex', gap: 10, alignItems: 'center' }}>
        <input
          style={{ ...inputStyle, flex: 1 }}
          value={props.value}
          placeholder={props.placeholder}
          onChange={(event: ChangeEvent<HTMLInputElement>) => props.onChange(event.target.value)}
        />
        {props.onBrowse ? (
          <button type="button" style={subtleButtonStyle} onClick={props.onBrowse}>
            {props.browseLabel ?? 'Browse'}
          </button>
        ) : null}
      </div>
      {props.helperText ? <p style={helperStyle}>{props.helperText}</p> : null}
    </div>
  );
}

interface ProtonInstallOption {
  name: string;
  path: string;
  is_official: boolean;
}

function formatProtonInstallLabel(install: ProtonInstallOption, duplicateNameCounts: Record<string, number>): string {
  const baseLabel = install.name.trim() || 'Unnamed Proton install';
  if ((duplicateNameCounts[baseLabel] ?? 0) <= 1) {
    return baseLabel;
  }

  return `${baseLabel} (${install.is_official ? 'Steam' : 'Custom'})`;
}

function ProtonPathField(props: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder: string;
  installs: ProtonInstallOption[];
  error: string | null;
  onBrowse: () => Promise<void>;
}) {
  const duplicateNameCounts = props.installs.reduce<Record<string, number>>((counts, install) => {
    const key = install.name.trim() || 'Unnamed Proton install';
    counts[key] = (counts[key] ?? 0) + 1;
    return counts;
  }, {});
  const selectedInstallPath = props.installs.find((install) => install.path.trim() === props.value.trim())?.path ?? '';

  return (
    <div style={{ ...fieldStyle, marginTop: 16 }}>
      <label style={labelStyle}>{props.label}</label>
      <div style={{ display: 'grid', gap: 10 }}>
        <select
          style={inputStyle}
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

        <div style={{ display: 'flex', gap: 10, alignItems: 'center' }}>
          <input
            style={{ ...inputStyle, flex: 1 }}
            value={props.value}
            placeholder={props.placeholder}
            onChange={(event: ChangeEvent<HTMLInputElement>) => props.onChange(event.target.value)}
          />
          <button type="button" style={subtleButtonStyle} onClick={props.onBrowse}>
            Browse
          </button>
        </div>
      </div>
      <p style={{ ...helperStyle, marginTop: 8 }}>
        Pick a detected Proton install to fill this field automatically, or edit the path manually.
      </p>
      {props.error ? <p style={{ ...helperStyle, marginTop: 8, color: '#ffb4b4' }}>{props.error}</p> : null}
    </div>
  );
}

async function chooseFile(title: string, filters?: { name: string; extensions: string[] }[]) {
  const result = await open({
    directory: false,
    multiple: false,
    title,
    filters,
  });

  if (Array.isArray(result)) {
    return result[0] ?? null;
  }

  return result ?? null;
}

async function chooseDirectory(title: string) {
  const result = await open({
    directory: true,
    multiple: false,
    title,
  });

  if (Array.isArray(result)) {
    return result[0] ?? null;
  }

  return result ?? null;
}

function deriveSteamClientInstallPath(compatdataPath: string): string {
  const marker = '/steamapps/compatdata/';
  const normalized = compatdataPath.trim().replace(/\\/g, '/');
  const index = normalized.indexOf(marker);

  return index >= 0 ? normalized.slice(0, index) : '';
}

export interface ProfileEditorProps {
  state: UseProfileResult;
  onEditorTabChange?: (tab: 'profile' | 'install') => void;
}

export function ProfileEditorView({ state, onEditorTabChange }: ProfileEditorProps) {
  const {
    profiles,
    selectedProfile,
    profileName,
    profile,
    dirty,
    loading,
    saving,
    deleting,
    error,
    profileExists,
    pendingDelete,
    setProfileName,
    selectProfile,
    hydrateProfile,
    updateProfile,
    saveProfile,
    deleteProfile,
    confirmDelete,
    executeDelete,
    cancelDelete,
    refreshProfiles,
  } = state;

  const canSave =
    profileName.trim().length > 0 && profile.game.executable_path.trim().length > 0 && !saving && !deleting && !loading;
  const canDelete = profileExists && !saving && !deleting && !loading;
  const launchMethod = profile.launch.method || 'proton_run';
  const supportsTrainerLaunch = launchMethod !== 'native';
  const steamClientInstallPath = deriveSteamClientInstallPath(profile.steam.compatdata_path);
  const [editorTab, setEditorTab] = useState<'profile' | 'install'>('profile');
  const [protonInstalls, setProtonInstalls] = useState<ProtonInstallOption[]>([]);
  const [protonInstallsError, setProtonInstallsError] = useState<string | null>(null);

  useEffect(() => {
    onEditorTabChange?.(editorTab);
  }, [editorTab, onEditorTabChange]);

  function handleInstallReview(profileNameValue: string, generatedProfile: GameProfile) {
    hydrateProfile(profileNameValue, generatedProfile);
    setEditorTab('profile');
  }

  useEffect(() => {
    let active = true;

    async function loadProtonInstalls() {
      try {
        const installs = await invoke<ProtonInstallOption[]>('list_proton_installs', {
          steamClientInstallPath: steamClientInstallPath.trim().length > 0 ? steamClientInstallPath : undefined,
        });
        const sortedInstalls = [...installs].sort((left, right) => {
          if (left.is_official !== right.is_official) {
            return left.is_official ? -1 : 1;
          }

          return left.name.localeCompare(right.name) || left.path.localeCompare(right.path);
        });

        if (!active) {
          return;
        }

        setProtonInstalls(sortedInstalls);
        setProtonInstallsError(null);
      } catch (loadError) {
        if (!active) {
          return;
        }

        setProtonInstalls([]);
        setProtonInstallsError(loadError instanceof Error ? loadError.message : String(loadError));
      }
    }

    void loadProtonInstalls();

    return () => {
      active = false;
    };
  }, [steamClientInstallPath]);

  return (
    <section style={panelStyle}>
      <div style={{ display: 'flex', justifyContent: 'space-between', gap: 16, alignItems: 'center' }}>
        <div style={{ display: 'grid', gap: 6 }}>
          <h2 style={{ margin: 0, fontSize: 18 }}>Profile</h2>
          <p style={helperStyle}>Select an existing profile or type a new name before saving.</p>
        </div>
        <button type="button" style={subtleButtonStyle} onClick={() => void refreshProfiles()}>
          Refresh
        </button>
      </div>

      <div className="crosshook-subtab-row" role="tablist" aria-label="Profile editor sections">
        <button
          type="button"
          className={`crosshook-subtab ${editorTab === 'profile' ? 'crosshook-subtab--active' : ''}`}
          role="tab"
          aria-selected={editorTab === 'profile'}
          onClick={() => setEditorTab('profile')}
        >
          Profile
        </button>
        <button
          type="button"
          className={`crosshook-subtab ${editorTab === 'install' ? 'crosshook-subtab--active' : ''}`}
          role="tab"
          aria-selected={editorTab === 'install'}
          onClick={() => setEditorTab('install')}
        >
          Install Game
        </button>
      </div>

      {editorTab === 'profile' ? (
        <div className="crosshook-profile-shell">
          <div className="crosshook-install-section-title">Profile Identity</div>
          <div style={{ display: 'grid', gap: 12, gridTemplateColumns: '1fr auto' }}>
            <div style={fieldStyle}>
              <label style={labelStyle}>Profile Name</label>
              <input
                style={inputStyle}
                list="crosshook-profiles"
                value={profileName}
                placeholder="Enter or choose a profile name"
                onChange={(event) => setProfileName(event.target.value)}
              />
              <datalist id="crosshook-profiles">
                {profiles.map((name) => (
                  <option key={name} value={name} />
                ))}
              </datalist>
            </div>

            <div style={fieldStyle}>
              <label style={labelStyle}>Load Profile</label>
              <select
                style={inputStyle}
                value={selectedProfile}
                onChange={(event) => void selectProfile(event.target.value)}
              >
                <option value="">Create New</option>
                {profiles.map((name) => (
                  <option key={name} value={name}>
                    {name}
                  </option>
                ))}
              </select>
            </div>
          </div>

          <div className="crosshook-install-section-title">Game</div>
          <div style={{ display: 'grid', gap: 14, gridTemplateColumns: 'repeat(2, minmax(0, 1fr))', marginTop: 16 }}>
            <FieldRow
              label="Game Name"
              value={profile.game.name}
              onChange={(value) =>
                updateProfile((current) => ({
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
                updateProfile((current) => ({
                  ...current,
                  game: { ...current.game, executable_path: value },
                }))
              }
              placeholder="/path/to/game.exe"
              browseLabel="Browse"
              onBrowse={async () => {
                const path =
                  launchMethod === 'native'
                    ? await chooseFile('Select Linux Game Executable')
                    : await chooseFile('Select Game Executable', [{ name: 'Windows Executable', extensions: ['exe'] }]);

                if (path) {
                  updateProfile((current) => ({
                    ...current,
                    game: { ...current.game, executable_path: path },
                  }));
                }
              }}
            />
          </div>

          <div style={{ ...fieldStyle, marginTop: 16 }}>
            <label style={labelStyle}>Runner Method</label>
            <select
              style={inputStyle}
              value={launchMethod}
              onChange={(event) =>
                updateProfile((current) => ({
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
            <p style={helperStyle}>
              Choose the runner explicitly so CrossHook saves the correct launch method and only shows the relevant
              fields.
            </p>
          </div>

          {supportsTrainerLaunch ? <div className="crosshook-install-section-title">Trainer</div> : null}
          {supportsTrainerLaunch ? (
            <div style={{ display: 'grid', gap: 14, gridTemplateColumns: 'repeat(2, minmax(0, 1fr))', marginTop: 16 }}>
              <FieldRow
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
                onBrowse={async () => {
                  const path = await chooseFile('Select Trainer Executable', [
                    { name: 'Windows Executable', extensions: ['exe'] },
                  ]);

                  if (path) {
                    updateProfile((current) => ({
                      ...current,
                      trainer: { ...current.trainer, path },
                    }));
                  }
                }}
              />
            </div>
          ) : null}

          <div className="crosshook-install-section-title">
            {launchMethod === 'steam_applaunch'
              ? 'Steam Runtime'
              : launchMethod === 'proton_run'
                ? 'Proton Runtime'
                : 'Native Runtime'}
          </div>
          {launchMethod === 'steam_applaunch' ? (
            <>
              <div
                style={{ display: 'grid', gap: 14, gridTemplateColumns: 'repeat(2, minmax(0, 1fr))', marginTop: 16 }}
              >
                <FieldRow
                  label="Steam App ID"
                  value={profile.steam.app_id}
                  onChange={(value) =>
                    updateProfile((current) => ({
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
                    updateProfile((current) => ({
                      ...current,
                      steam: { ...current.steam, compatdata_path: value },
                    }))
                  }
                  placeholder="/home/user/.local/share/Steam/steamapps/compatdata/1245620"
                  browseLabel="Browse"
                  onBrowse={async () => {
                    const path = await chooseDirectory('Select Steam Prefix Directory');

                    if (path) {
                      updateProfile((current) => ({
                        ...current,
                        steam: { ...current.steam, compatdata_path: path },
                      }));
                    }
                  }}
                />

                <FieldRow
                  label="Launcher Name"
                  value={profile.steam.launcher.display_name}
                  onChange={(value) =>
                    updateProfile((current) => ({
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
                  value={profile.steam.launcher.icon_path}
                  onChange={(value) =>
                    updateProfile((current) => ({
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
                      updateProfile((current) => ({
                        ...current,
                        steam: {
                          ...current.steam,
                          launcher: { ...current.steam.launcher, icon_path: path },
                        },
                      }));
                    }
                  }}
                />
              </div>

              <ProtonPathField
                label="Proton Path"
                value={profile.steam.proton_path}
                onChange={(value) =>
                  updateProfile((current) => ({
                    ...current,
                    steam: { ...current.steam, proton_path: value },
                  }))
                }
                placeholder="/home/user/.steam/root/steamapps/common/Proton - Experimental/proton"
                installs={protonInstalls}
                error={protonInstallsError}
                onBrowse={async () => {
                  const path = await chooseFile('Select Proton Executable');

                  if (path) {
                    updateProfile((current) => ({
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
          ) : null}

          {launchMethod === 'proton_run' ? (
            <>
              <div
                style={{ display: 'grid', gap: 14, gridTemplateColumns: 'repeat(2, minmax(0, 1fr))', marginTop: 16 }}
              >
                <FieldRow
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
                  onBrowse={async () => {
                    const path = await chooseDirectory('Select Proton Prefix Directory');

                    if (path) {
                      updateProfile((current) => ({
                        ...current,
                        runtime: { ...current.runtime, prefix_path: path },
                      }));
                    }
                  }}
                />

                <FieldRow
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
                  onBrowse={async () => {
                    const path = await chooseDirectory('Select Working Directory');

                    if (path) {
                      updateProfile((current) => ({
                        ...current,
                        runtime: { ...current.runtime, working_directory: path },
                      }));
                    }
                  }}
                />
              </div>

              <ProtonPathField
                label="Proton Path"
                value={profile.runtime.proton_path}
                onChange={(value) =>
                  updateProfile((current) => ({
                    ...current,
                    runtime: { ...current.runtime, proton_path: value },
                  }))
                }
                placeholder="/path/to/proton"
                installs={protonInstalls}
                error={protonInstallsError}
                onBrowse={async () => {
                  const path = await chooseFile('Select Proton Executable');

                  if (path) {
                    updateProfile((current) => ({
                      ...current,
                      runtime: { ...current.runtime, proton_path: path },
                    }));
                  }
                }}
              />
            </>
          ) : null}

          {launchMethod === 'native' ? (
            <div style={{ display: 'grid', gap: 14, gridTemplateColumns: 'repeat(2, minmax(0, 1fr))', marginTop: 16 }}>
              <FieldRow
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
                onBrowse={async () => {
                  const path = await chooseDirectory('Select Working Directory');

                  if (path) {
                    updateProfile((current) => ({
                      ...current,
                      runtime: { ...current.runtime, working_directory: path },
                    }));
                  }
                }}
              />
            </div>
          ) : null}

          <div style={{ display: 'flex', gap: 12, flexWrap: 'wrap', marginTop: 18 }}>
            <button type="button" style={buttonStyle} onClick={() => void saveProfile()} disabled={!canSave}>
              {saving ? 'Saving...' : 'Save'}
            </button>
            <button
              type="button"
              style={subtleButtonStyle}
              onClick={() => void confirmDelete(profileName)}
              disabled={!canDelete}
            >
              {deleting ? 'Deleting...' : 'Delete'}
            </button>
            <div style={{ display: 'flex', alignItems: 'center', color: dirty ? '#ffd166' : '#9bb1c8' }}>
              {loading ? 'Loading...' : dirty ? 'Unsaved changes' : 'No unsaved changes'}
            </div>
          </div>

          {error ? (
            <div
              style={{
                marginTop: 16,
                borderRadius: 12,
                padding: 12,
                background: 'rgba(140, 40, 40, 0.2)',
                border: '1px solid rgba(255, 90, 90, 0.3)',
                color: '#ffd4d4',
              }}
            >
              {error}
            </div>
          ) : null}
        </div>
      ) : (
        <InstallGamePanel onReviewGeneratedProfile={handleInstallReview} />
      )}

      {pendingDelete && (
        <div
          style={{
            position: 'fixed',
            inset: 0,
            background: 'rgba(0,0,0,0.5)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            zIndex: 1000,
          }}
        >
          <div
            style={{
              background: '#1a1a2e',
              borderRadius: '12px',
              padding: '24px',
              maxWidth: '480px',
              width: '90%',
            }}
          >
            <h3 style={{ margin: '0 0 12px' }}>Delete Profile</h3>
            <p>
              Delete profile <strong>{pendingDelete.name}</strong>?
            </p>
            {pendingDelete.launcherInfo && (
              <div
                style={{
                  background: 'rgba(245, 158, 11, 0.08)',
                  padding: '10px',
                  borderRadius: '6px',
                  fontSize: '0.85rem',
                  marginBottom: '12px',
                }}
              >
                <p style={{ margin: '0 0 6px', fontWeight: 600 }}>Launcher files will also be removed:</p>
                <p style={{ margin: '2px 0', color: '#d1d5db', wordBreak: 'break-all' }}>
                  {pendingDelete.launcherInfo.script_path}
                </p>
                <p style={{ margin: '2px 0', color: '#d1d5db', wordBreak: 'break-all' }}>
                  {pendingDelete.launcherInfo.desktop_entry_path}
                </p>
              </div>
            )}
            <div style={{ display: 'flex', gap: '10px', justifyContent: 'flex-end' }}>
              <button type="button" onClick={cancelDelete} style={{ minHeight: '44px', padding: '8px 20px' }}>
                Cancel
              </button>
              <button
                type="button"
                onClick={() => void executeDelete()}
                style={{
                  minHeight: '44px',
                  padding: '8px 20px',
                  background: 'rgba(185, 28, 28, 0.16)',
                  border: '1px solid rgba(248, 113, 113, 0.28)',
                  color: '#fee2e2',
                  borderRadius: '6px',
                  cursor: 'pointer',
                }}
              >
                {pendingDelete.launcherInfo ? 'Delete Profile and Launcher' : 'Delete Profile'}
              </button>
            </div>
          </div>
        </div>
      )}
    </section>
  );
}

export function ProfileEditor() {
  const state = useProfile();
  return (
    <div
      style={{
        minHeight: '100vh',
        padding: 24,
        background:
          'radial-gradient(circle at top, rgba(27, 59, 108, 0.35), transparent 35%), linear-gradient(180deg, #08111c 0%, #0b1320 100%)',
        color: '#f3f6fb',
      }}
    >
      <div style={{ display: 'grid', gap: 18, maxWidth: 1180, margin: '0 auto' }}>
        <header style={{ display: 'grid', gap: 8 }}>
          <div style={{ color: '#60a5fa', fontSize: 12, letterSpacing: '0.2em', textTransform: 'uppercase' }}>
            CrossHook Native
          </div>
          <h1 style={{ margin: 0, fontSize: 32, fontWeight: 800 }}>Profile Editor</h1>
          <p style={{ ...helperStyle, maxWidth: 760 }}>
            Edit a profile, save it to Tauri storage, and configure the correct Steam, Proton, or native runner path.
          </p>
        </header>
        <ProfileEditorView state={state} />
      </div>
    </div>
  );
}

export default ProfileEditor;
