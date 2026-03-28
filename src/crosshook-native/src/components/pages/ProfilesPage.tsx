import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

import LauncherExport from '../LauncherExport';
import ProfileActions from '../ProfileActions';
import ProfileFormSections, { type ProtonInstallOption } from '../ProfileFormSections';
import ProfilePreviewModal from '../ProfilePreviewModal';
import { CollapsibleSection } from '../ui/CollapsibleSection';
import { HealthBadge } from '../HealthBadge';
import { usePreferencesContext } from '../../context/PreferencesContext';
import { useProfileContext } from '../../context/ProfileContext';
import { useProfileHealth } from '../../hooks/useProfileHealth';
import { PageBanner, ProfilesArt } from '../layout/PageBanner';
import { deriveTargetHomePath } from '../../utils/steam';
import type { EnrichedProfileHealthReport } from '../../types';

interface RenameToast {
  newName: string;
  oldName: string;
}

function formatRelativeTime(isoString: string): string {
  const then = new Date(isoString).getTime();
  const nowMs = new Date().getTime();
  const diffDays = Math.floor((nowMs - then) / (1000 * 60 * 60 * 24));

  if (diffDays <= 0) return 'today';
  if (diffDays === 1) return 'yesterday';
  if (diffDays < 7) return `${diffDays} days ago`;
  if (diffDays < 30) {
    const weeks = Math.floor(diffDays / 7);
    return `${weeks} week${weeks !== 1 ? 's' : ''} ago`;
  }
  const months = Math.floor(diffDays / 30);
  return `${months} month${months !== 1 ? 's' : ''} ago`;
}

const RENAME_TOAST_DURATION_MS = 6000;
const HEALTH_BANNER_DISMISSED_SESSION_KEY = 'crosshook.healthBannerDismissed';
const RENAME_TOAST_DISMISSED_SESSION_KEY = 'crosshook.renameToastDismissed';

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
  const [healthBannerDismissed, setHealthBannerDismissed] = useState(() => {
    try {
      return sessionStorage.getItem(HEALTH_BANNER_DISMISSED_SESSION_KEY) === '1';
    } catch {
      return false;
    }
  });
  const [renameToastDismissed, setRenameToastDismissed] = useState(() => {
    try {
      return sessionStorage.getItem(RENAME_TOAST_DISMISSED_SESSION_KEY) === '1';
    } catch {
      return false;
    }
  });
  const renameToastTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [pendingLauncherReExport, setPendingLauncherReExport] = useState(false);
  const [showProfilePreview, setShowProfilePreview] = useState(false);
  const [profilePreviewContent, setProfilePreviewContent] = useState('');
  const [previewing, setPreviewing] = useState(false);
  const [previewError, setPreviewError] = useState<string | null>(null);

  const { batchValidate, revalidateSingle, healthByName, summary, loading: healthLoading } = useProfileHealth();

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
  const canPreview = profileName.trim().length > 0 && !loading;
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

    setRenameToastDismissed(false);
    try {
      sessionStorage.removeItem(RENAME_TOAST_DISMISSED_SESSION_KEY);
    } catch {
      // Ignore storage errors in restricted environments.
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
    setRenameToastDismissed(true);
    try {
      sessionStorage.setItem(RENAME_TOAST_DISMISSED_SESSION_KEY, '1');
    } catch {
      // Ignore storage errors in restricted environments.
    }
  }, []);

  const dismissHealthBanner = useCallback(() => {
    setHealthBannerDismissed(true);
    try {
      sessionStorage.setItem(HEALTH_BANNER_DISMISSED_SESSION_KEY, '1');
    } catch {
      // Ignore storage errors in restricted environments.
    }
  }, []);

  const handleSave = useCallback(async () => {
    await saveProfile();
    if (profileName.trim()) {
      void revalidateSingle(profileName.trim());
    }
  }, [saveProfile, profileName, revalidateSingle]);

  const undoRename = useCallback(() => {
    if (!renameToast) {
      return;
    }

    const { oldName, newName } = renameToast;
    dismissRenameToast();
    void renameProfile(newName, oldName).then(({ ok, hadLauncher }) => {
      if (!ok) {
        return;
      }

      if (hadLauncher) {
        setPendingLauncherReExport(true);
      }
    });
  }, [dismissRenameToast, renameProfile, renameToast]);

  const handleRenameConfirm = useCallback((oldName: string, newName: string) => {
    setPendingRename(null);
    void renameProfile(oldName, newName).then(({ ok, hadLauncher }) => {
      if (!ok) {
        return;
      }

      showRenameToast(oldName, newName);
      if (hadLauncher) {
        setPendingLauncherReExport(true);
      }
    });
  }, [renameProfile, showRenameToast]);

  async function handlePreviewProfile() {
    setPreviewing(true);
    setPreviewError(null);
    try {
      const toml = await invoke<string>('profile_export_toml', {
        name: profileName,
        data: profile,
      });
      setProfilePreviewContent(toml);
      setPreviewError(null);
      setShowProfilePreview(true);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      console.error('Profile preview failed:', err);
      setPreviewError(message);
    } finally {
      setPreviewing(false);
    }
  }

  function handleCloseProfilePreview() {
    setShowProfilePreview(false);
    setPreviewError(null);
  }

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

      {summary !== null && !healthLoading && summary.broken_count > 0 && !healthBannerDismissed ? (
        <div
          className="crosshook-rename-toast"
          role="status"
          aria-live="polite"
        >
          <span>
            {summary.broken_count} profile{summary.broken_count !== 1 ? 's' : ''} have issues that may prevent launching
          </span>
          <button
            type="button"
            className="crosshook-rename-toast-dismiss"
            onClick={dismissHealthBanner}
            aria-label="Dismiss"
          >
            &times;
          </button>
        </div>
      ) : null}

      <div style={{ display: 'grid', gap: 24 }}>
        <CollapsibleSection
          title="Profile"
          className="crosshook-panel"
          meta={
            <span style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              {selectedProfile && healthByName[selectedProfile] ? (
                <HealthBadge status={healthByName[selectedProfile].status} />
              ) : null}
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
            </span>
          }
        >
          <p className="crosshook-help-text">Edit the current profile, then save it before launching or exporting.</p>

          {summary !== null && (summary.stale_count + summary.broken_count) > 0 ? (
            <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 12 }}>
              <span className="crosshook-status-chip">
                {summary.stale_count + summary.broken_count} of {summary.total_count} profile{summary.total_count !== 1 ? 's' : ''} have issues
              </span>
              <button
                type="button"
                className="crosshook-button crosshook-button--secondary"
                disabled={healthLoading}
                onClick={() => void batchValidate()}
              >
                {healthLoading ? 'Checking...' : 'Re-check All'}
              </button>
            </div>
          ) : null}

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
            canPreview={canPreview}
            previewing={previewing}
            onSave={handleSave}
            onDelete={() => confirmDelete(profileName)}
            onDuplicate={() => duplicateProfile(profileName)}
            onRename={() => {
              setPendingRename(selectedProfile);
              setRenameValue(selectedProfile);
            }}
            onPreview={handlePreviewProfile}
          />
          {previewError ? (
            <p className="crosshook-danger" role="alert" style={{ marginTop: 12 }}>
              Preview failed: {previewError}
            </p>
          ) : null}

          {(() => {
            const report = selectedProfile ? healthByName[selectedProfile] : undefined;
            if (!report || (report.status !== 'broken' && report.status !== 'stale') || report.issues.length === 0) {
              return null;
            }

            const enriched = report as EnrichedProfileHealthReport;
            const metadata = enriched.metadata ?? null;

            const driftMessage: Record<string, string> = {
              missing: 'Exported launcher not found — re-export recommended',
              moved: 'Exported launcher has moved — re-export recommended',
              stale: 'Exported launcher may be outdated — re-export recommended',
            };
            const driftWarning =
              metadata !== null && metadata.launcher_drift_state !== null
                ? driftMessage[metadata.launcher_drift_state] ?? null
                : null;

            return (
              <CollapsibleSection title="Health Issues" className="crosshook-panel">
                {metadata !== null ? (
                  <div style={{ marginBottom: 10, display: 'grid', gap: 4 }}>
                    {metadata.last_success !== null ? (
                      <p className="crosshook-help-text" style={{ margin: 0 }}>
                        Last worked: {formatRelativeTime(metadata.last_success)}
                      </p>
                    ) : null}
                    {metadata.total_launches > 0 ? (
                      <p className="crosshook-help-text" style={{ margin: 0 }}>
                        Launched {metadata.total_launches} time{metadata.total_launches !== 1 ? 's' : ''}{' '}
                        &bull; {metadata.failure_count_30d} failure{metadata.failure_count_30d !== 1 ? 's' : ''} in last 30 days
                      </p>
                    ) : null}
                    {driftWarning !== null ? (
                      <p className="crosshook-danger" style={{ margin: 0 }} role="alert">
                        {driftWarning}
                      </p>
                    ) : null}
                    {metadata.is_community_import && (report.status === 'broken' || report.status === 'stale') ? (
                      <p className="crosshook-help-text" style={{ margin: 0 }}>
                        This profile was imported from a community tap — paths may need adjustment for your system.
                      </p>
                    ) : null}
                  </div>
                ) : null}
                <ul style={{ margin: 0, padding: 0, listStyle: 'none', display: 'grid', gap: 8 }}>
                  {report.issues.map((issue, index) => (
                    <li key={index} style={{ borderLeft: '3px solid var(--crosshook-danger, #ef4444)', paddingLeft: 10 }}>
                      <strong>{issue.field}</strong>
                      {issue.path ? <span className="crosshook-muted"> — {issue.path}</span> : null}
                      <p style={{ margin: '2px 0' }}>{issue.message}</p>
                      {issue.remediation ? (
                        <p className="crosshook-help-text" style={{ margin: '2px 0' }}>{issue.remediation}</p>
                      ) : null}
                    </li>
                  ))}
                </ul>
              </CollapsibleSection>
            );
          })()}
        </CollapsibleSection>

        {supportsLauncherExport ? (
          <CollapsibleSection title="Launcher Export" className="crosshook-panel">
            <LauncherExport
              profile={profile}
              profileName={profileName}
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
                    handleRenameConfirm(oldName, newName);
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
                  handleRenameConfirm(oldName, newName);
                }}
              >
                {renaming ? 'Renaming...' : 'Rename'}
              </button>
            </div>
          </div>
        </div>
      ) : null}

      {renameToast && !renameToastDismissed ? (
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

      {showProfilePreview ? (
        <ProfilePreviewModal
          tomlContent={profilePreviewContent}
          profileName={profileName}
          onClose={handleCloseProfilePreview}
        />
      ) : null}
    </>
  );
}

export default ProfilesPage;
