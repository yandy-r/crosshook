import { useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

import LauncherExport from '../LauncherExport';
import ProfileActions from '../ProfileActions';
import ProfileFormSections, { type ProtonInstallOption } from '../ProfileFormSections';
import { CollapsibleSection } from '../ui/CollapsibleSection';
import { usePreferencesContext } from '../../context/PreferencesContext';
import { useProfileContext } from '../../context/ProfileContext';
import { PageBanner, ProfilesArt } from '../layout/PageBanner';
import { deriveTargetHomePath } from '../../utils/steam';

function sortProtonInstalls(installs: ProtonInstallOption[]): ProtonInstallOption[] {
  return [...installs].sort((left, right) => {
    if (left.is_official !== right.is_official) {
      return left.is_official ? -1 : 1;
    }

    return left.name.localeCompare(right.name) || left.path.localeCompare(right.path);
  });
}

export function ProfilesPage() {
  const {
    defaultSteamClientInstallPath,
  } = usePreferencesContext();
  const {
    deleting,
    dirty,
    duplicateProfile,
    duplicating,
    error,
    executeDelete,
    loading,
    pendingDelete,
    profile,
    profileExists,
    profileName,
    profiles,
    refreshProfiles,
    saveProfile,
    saving,
    selectProfile,
    selectedProfile,
    setProfileName,
    cancelDelete,
    confirmDelete,
    updateProfile,
    launchMethod,
    steamClientInstallPath,
  } = useProfileContext();
  const [protonInstalls, setProtonInstalls] = useState<ProtonInstallOption[]>([]);
  const [protonInstallsError, setProtonInstallsError] = useState<string | null>(null);

  const effectiveSteamClientInstallPath = useMemo(
    () => defaultSteamClientInstallPath || steamClientInstallPath,
    [defaultSteamClientInstallPath, steamClientInstallPath],
  );
  const targetHomePath = useMemo(
    () => deriveTargetHomePath(effectiveSteamClientInstallPath),
    [effectiveSteamClientInstallPath],
  );
  const canSave =
    profileName.trim().length > 0 &&
    profile.game.executable_path.trim().length > 0 &&
    !saving &&
    !deleting &&
    !loading;
  const canDelete = profileExists && !saving && !deleting && !loading && !duplicating;
  const canDuplicate = profileExists && !saving && !deleting && !loading && !duplicating;
  const supportsLauncherExport = launchMethod === 'steam_applaunch' || launchMethod === 'proton_run';

  useEffect(() => {
    let active = true;

    async function loadProtonInstalls() {
      try {
        const installs = await invoke<ProtonInstallOption[]>('list_proton_installs', {
          steamClientInstallPath:
            effectiveSteamClientInstallPath.trim().length > 0
              ? effectiveSteamClientInstallPath
              : undefined,
        });

        if (!active) {
          return;
        }

        setProtonInstalls(sortProtonInstalls(installs));
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
  }, [effectiveSteamClientInstallPath]);

  return (
    <>
      <PageBanner
        eyebrow="Profiles"
        title="Profile editor"
        copy="Select an existing profile or build a new one, then save it before switching to launch or export workflows."
        illustration={<ProfilesArt />}
      />

      <div style={{ display: 'grid', gap: 24 }}>
        <CollapsibleSection
          title="Profile"
          className="crosshook-panel"
          meta={
            <button
              type="button"
              className="crosshook-button crosshook-button--secondary"
              onClick={(event) => {
                event.preventDefault();
                void refreshProfiles();
              }}
            >
              Refresh
            </button>
          }
        >
          <p className="crosshook-help-text">Edit the current profile, then save it before launching or exporting.</p>

          <ProfileFormSections
            profileName={profileName}
            profile={profile}
            launchMethod={launchMethod}
            protonInstalls={protonInstalls}
            protonInstallsError={protonInstallsError}
            profileSelector={{
              profiles,
              selectedProfile,
              onSelectProfile: selectProfile,
            }}
            onProfileNameChange={setProfileName}
            onUpdateProfile={updateProfile}
          />

          <ProfileActions
            dirty={dirty}
            loading={loading}
            saving={saving}
            deleting={deleting}
            duplicating={duplicating}
            error={error}
            canSave={canSave}
            canDelete={canDelete}
            canDuplicate={canDuplicate}
            onSave={saveProfile}
            onDelete={() => confirmDelete(profileName)}
            onDuplicate={() => duplicateProfile(selectedProfile)}
          />
        </CollapsibleSection>

        {supportsLauncherExport ? (
          <CollapsibleSection title="Launcher Export" className="crosshook-panel">
            <LauncherExport
              profile={profile}
              method={launchMethod}
              steamClientInstallPath={effectiveSteamClientInstallPath}
              targetHomePath={targetHomePath}
            />
          </CollapsibleSection>
        ) : null}
      </div>

      {pendingDelete ? (
        <div className="crosshook-profile-editor-delete-overlay" data-crosshook-focus-root="modal">
          <div className="crosshook-profile-editor-delete-dialog">
            <h3 style={{ margin: '0 0 12px' }}>Delete Profile</h3>
            <p>
              Delete profile <strong>{pendingDelete.name}</strong>?
            </p>
            {pendingDelete.launcherInfo ? (
              <div className="crosshook-profile-editor-delete-warning">
                <p style={{ margin: '0 0 6px', fontWeight: 600 }}>Launcher files will also be removed:</p>
                <p style={{ margin: '2px 0', color: '#d1d5db', wordBreak: 'break-all' }}>
                  {pendingDelete.launcherInfo.script_path}
                </p>
                <p style={{ margin: '2px 0', color: '#d1d5db', wordBreak: 'break-all' }}>
                  {pendingDelete.launcherInfo.desktop_entry_path}
                </p>
              </div>
            ) : null}
            <div className="crosshook-profile-editor-delete-actions">
              <button
                type="button"
                className="crosshook-button crosshook-button--secondary"
                onClick={cancelDelete}
                data-crosshook-modal-close
              >
                Cancel
              </button>
              <button
                type="button"
                className="crosshook-profile-editor-delete-confirm"
                onClick={() => void executeDelete()}
              >
                {pendingDelete.launcherInfo ? 'Delete Profile and Launcher' : 'Delete Profile'}
              </button>
            </div>
          </div>
        </div>
      ) : null}
    </>
  );
}

export default ProfilesPage;
