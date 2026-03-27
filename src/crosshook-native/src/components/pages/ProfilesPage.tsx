import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

import LauncherExport from '../LauncherExport';
import ProfileActions from '../ProfileActions';
import ProfileFormSections, { type ProtonInstallOption } from '../ProfileFormSections';
import { CollapsibleSection } from '../ui/CollapsibleSection';
import { usePreferencesContext } from '../../context/PreferencesContext';
import { useProfileContext } from '../../context/ProfileContext';
import { PageBanner, ProfilesArt } from '../layout/PageBanner';
import { deriveTargetHomePath } from '../../utils/steam';

interface RenameToast {
  newName: string;
  oldName: string;
}

const RENAME_TOAST_DURATION_MS = 6000;

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
    renameProfile,
    renaming,
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
  const [pendingRename, setPendingRename] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState('');
  const renameInputRef = useRef<HTMLInputElement>(null);
  const [renameToast, setRenameToast] = useState<RenameToast | null>(null);
  const renameToastTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [pendingLauncherReExport, setPendingLauncherReExport] = useState(false);

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
  const canDelete = profileExists && !saving && !deleting && !loading && !duplicating && !renaming;
  const canDuplicate = profileExists && !saving && !deleting && !loading && !duplicating && !renaming;
  const canRename = profileExists && !saving && !deleting && !loading && !duplicating && !renaming;
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

  useEffect(() => {
    if (pendingRename !== null) {
      renameInputRef.current?.select();
    }
  }, [pendingRename]);

  // F2 keyboard shortcut: open rename dialog when a profile is selected and no modal is open
  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key !== 'F2') {
        return;
      }

      // Skip if focus is inside an editable element
      const target = event.target;
      if (
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        (target instanceof HTMLElement && target.isContentEditable)
      ) {
        return;
      }

      // Skip if a modal is already open
      if (pendingRename !== null || pendingDelete !== null) {
        return;
      }

      // Only open if a saved profile is selected and rename is allowed
      if (!canRename || !selectedProfile) {
        return;
      }

      event.preventDefault();
      setPendingRename(selectedProfile);
      setRenameValue(selectedProfile);
    }

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [canRename, pendingDelete, pendingRename, selectedProfile]);

  // Clean up toast timer on unmount
  useEffect(() => {
    return () => {
      if (renameToastTimerRef.current !== null) {
        clearTimeout(renameToastTimerRef.current);
      }
    };
  }, []);

  const showRenameToast = useCallback((oldName: string, newName: string) => {
    if (renameToastTimerRef.current !== null) {
      clearTimeout(renameToastTimerRef.current);
    }

    setRenameToast({ oldName, newName });
    renameToastTimerRef.current = setTimeout(() => {
      setRenameToast(null);
      renameToastTimerRef.current = null;
    }, RENAME_TOAST_DURATION_MS);
  }, []);

  const dismissRenameToast = useCallback(() => {
    if (renameToastTimerRef.current !== null) {
      clearTimeout(renameToastTimerRef.current);
      renameToastTimerRef.current = null;
    }

    setRenameToast(null);
  }, []);

  const undoRename = useCallback(() => {
    if (!renameToast) {
      return;
    }

    const { oldName, newName } = renameToast;
    dismissRenameToast();
    void renameProfile(newName, oldName);
  }, [dismissRenameToast, renameProfile, renameToast]);

  const renameNameTrimmed = renameValue.trim();
  const renameIsEmpty = renameNameTrimmed.length === 0;
  const renameIsUnchanged = pendingRename !== null && renameNameTrimmed === pendingRename;
  const renameHasConflict =
    !renameIsEmpty &&
    !renameIsUnchanged &&
    profiles.some((name) => name.toLowerCase() === renameNameTrimmed.toLowerCase());
  const renameError = renameIsEmpty
    ? 'Profile name cannot be empty.'
    : renameHasConflict
      ? `A profile named '${renameNameTrimmed}' already exists.`
      : null;
  const canConfirmRename = !renameIsEmpty && !renameIsUnchanged && !renameHasConflict && !renaming;

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
            profileExists={profileExists}
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
            renaming={renaming}
            error={error}
            canSave={canSave}
            canDelete={canDelete}
            canDuplicate={canDuplicate}
            canRename={canRename}
            onSave={saveProfile}
            onDelete={() => confirmDelete(profileName)}
            onDuplicate={() => duplicateProfile(profileName)}
            onRename={() => {
              setPendingRename(selectedProfile);
              setRenameValue(selectedProfile);
            }}
          />
        </CollapsibleSection>

        {supportsLauncherExport ? (
          <CollapsibleSection title="Launcher Export" className="crosshook-panel">
            <LauncherExport
              profile={profile}
              method={launchMethod}
              steamClientInstallPath={effectiveSteamClientInstallPath}
              targetHomePath={targetHomePath}
              pendingReExport={pendingLauncherReExport}
              onReExportHandled={() => setPendingLauncherReExport(false)}
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

      {pendingRename !== null ? (
        <div className="crosshook-profile-editor-delete-overlay" data-crosshook-focus-root="modal">
          <div
            className="crosshook-profile-editor-delete-dialog"
            role="dialog"
            aria-modal="true"
            aria-labelledby="rename-dialog-heading"
            style={{ marginBottom: 'auto', marginTop: '12vh' }}
          >
            <h3 id="rename-dialog-heading" style={{ margin: '0 0 12px' }}>Rename Profile</h3>
            <div className="crosshook-field">
              <label className="crosshook-label" htmlFor="rename-profile-input">
                New Name
              </label>
              <input
                id="rename-profile-input"
                ref={renameInputRef}
                className="crosshook-input"
                value={renameValue}
                onChange={(event) => setRenameValue(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === 'Enter' && canConfirmRename) {
                    const oldName = pendingRename;
                    const newName = renameNameTrimmed;
                    setPendingRename(null);
                    void renameProfile(oldName, newName).then((hadLauncher) => {
                      showRenameToast(oldName, newName);
                      if (hadLauncher) setPendingLauncherReExport(true);
                    });
                  }

                  if (event.key === 'Escape') {
                    setPendingRename(null);
                  }
                }}
              />
              {renameError ? <p className="crosshook-danger" role="alert">{renameError}</p> : null}
            </div>
            <div className="crosshook-profile-editor-delete-actions">
              <button
                type="button"
                className="crosshook-button crosshook-button--secondary"
                onClick={() => setPendingRename(null)}
                data-crosshook-modal-close
              >
                Cancel
              </button>
              <button
                type="button"
                className="crosshook-button"
                disabled={!canConfirmRename}
                onClick={() => {
                  const oldName = pendingRename;
                  const newName = renameNameTrimmed;
                  setPendingRename(null);
                  void renameProfile(oldName, newName).then((hadLauncher) => {
                    showRenameToast(oldName, newName);
                    if (hadLauncher) setPendingLauncherReExport(true);
                  });
                }}
              >
                {renaming ? 'Renaming...' : 'Rename'}
              </button>
            </div>
          </div>
        </div>
      ) : null}

      {renameToast ? (
        <div
          className="crosshook-rename-toast"
          role="status"
          aria-live="polite"
        >
          <span>Renamed to &lsquo;{renameToast.newName}&rsquo;</span>
          <button
            type="button"
            className="crosshook-button crosshook-button--ghost"
            onClick={undoRename}
          >
            Undo
          </button>
          <button
            type="button"
            className="crosshook-rename-toast-dismiss"
            onClick={dismissRenameToast}
            aria-label="Dismiss"
          >
            &times;
          </button>
        </div>
      ) : null}
    </>
  );
}

export default ProfilesPage;
