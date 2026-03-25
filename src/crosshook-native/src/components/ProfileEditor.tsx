import { useEffect, useMemo, useRef, useState, type Dispatch, type SetStateAction } from 'react';
import { invoke } from '@tauri-apps/api/core';
import InstallGamePanel from './InstallGamePanel';
import ProfileReviewModal, { type ProfileReviewModalConfirmation } from './ProfileReviewModal';
import ProfileFormSections, {
  deriveSteamClientInstallPath,
  type ProtonInstallOption,
} from './ProfileFormSections';
import { useProfile, type UseProfileResult } from '../hooks/useProfile';
import type { GameProfile } from '../types';
import type { InstallProfileReviewPayload } from '../types/install';
import type { ProfileReviewSession } from '../types/profile-review';
import { profilesEqual } from '../utils/profile-compare';

function updateProfileReviewSession(
  set: Dispatch<SetStateAction<ProfileReviewSession | null>>,
  updater: (session: ProfileReviewSession) => ProfileReviewSession,
) {
  set((current) => (current === null ? current : updater(current)));
}

function isProfileReviewSessionDirty(session: ProfileReviewSession): boolean {
  return (
    session.profileName.trim() !== session.originalProfileName.trim() ||
    !profilesEqual(session.draftProfile, session.originalProfile)
  );
}

type ReviewConfirmationState = ProfileReviewModalConfirmation & {
  restoreIsOpen: boolean;
};

function createProfileReviewSessionState(payload: InstallProfileReviewPayload): ProfileReviewSession {
  return {
    isOpen: true,
    source: payload.source,
    profileName: payload.profileName,
    originalProfileName: payload.profileName,
    originalProfile: payload.generatedProfile,
    draftProfile: payload.generatedProfile,
    candidateOptions: payload.candidateOptions,
    helperLogPath: payload.helperLogPath,
    installMessage: payload.message,
    saveError: null,
  };
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
    updateProfile,
    saveProfile,
    confirmDelete,
    executeDelete,
    cancelDelete,
    refreshProfiles,
  } = state;

  const canSave =
    profileName.trim().length > 0 && profile.game.executable_path.trim().length > 0 && !saving && !deleting && !loading;
  const canDelete = profileExists && !saving && !deleting && !loading;
  const launchMethod = profile.launch.method || 'proton_run';
  const steamClientInstallPath = deriveSteamClientInstallPath(profile.steam.compatdata_path);
  const [editorTab, setEditorTab] = useState<'profile' | 'install'>('profile');
  const [protonInstalls, setProtonInstalls] = useState<ProtonInstallOption[]>([]);
  const [protonInstallsError, setProtonInstallsError] = useState<string | null>(null);
  const [profileReviewSession, setProfileReviewSession] = useState<ProfileReviewSession | null>(null);
  const [reviewConfirmation, setReviewConfirmation] = useState<ReviewConfirmationState | null>(null);
  const reviewConfirmationResolverRef = useRef<((confirmed: boolean) => void) | null>(null);

  useEffect(() => {
    onEditorTabChange?.(editorTab);
  }, [editorTab, onEditorTabChange]);

  function resolveReviewConfirmation(confirmed: boolean) {
    const confirmation = reviewConfirmation;
    const resolver = reviewConfirmationResolverRef.current;

    reviewConfirmationResolverRef.current = null;
    setReviewConfirmation(null);

    if (confirmation === null) {
      resolver?.(confirmed);
      return;
    }

    if (confirmed) {
      confirmation.onConfirm();
    } else {
      updateProfileReviewSession(setProfileReviewSession, (current) => ({
        ...current,
        isOpen: confirmation.restoreIsOpen,
      }));
      confirmation.onCancel();
    }

    resolver?.(confirmed);
  }

  function requestReviewConfirmation(confirmation: ReviewConfirmationState) {
    if (reviewConfirmationResolverRef.current !== null) {
      return Promise.resolve(false);
    }

    setEditorTab('install');
    updateProfileReviewSession(setProfileReviewSession, (current) => ({
      ...current,
      isOpen: true,
    }));

    setReviewConfirmation(confirmation);

    return new Promise<boolean>((resolve) => {
      reviewConfirmationResolverRef.current = resolve;
    });
  }

  async function handleOpenProfileReview(payload: InstallProfileReviewPayload) {
    if (payload.source === 'manual-verify') {
      const currentSession = profileReviewSession;
      const sameReviewResult =
        currentSession !== null && currentSession.helperLogPath === payload.helperLogPath;

      if (currentSession !== null && !sameReviewResult) {
        if (isProfileReviewSessionDirty(currentSession)) {
          return requestReviewConfirmation({
            title: 'Open the latest review draft?',
            body: `A newer install result is ready for ${payload.profileName}. Opening it will discard the unsaved review draft that is currently loaded.`,
            confirmLabel: 'Open latest draft',
            cancelLabel: 'Keep current draft',
            tone: 'warning',
            restoreIsOpen: currentSession.isOpen,
            onConfirm: () => {
              setProfileReviewSession(createProfileReviewSessionState(payload));
              setEditorTab('install');
            },
            onCancel: () => {
              updateProfileReviewSession(setProfileReviewSession, (current) => ({
                ...current,
                isOpen: true,
              }));
            },
          });
        }

        setProfileReviewSession(createProfileReviewSessionState(payload));
        setEditorTab('install');
        return true;
      }

      setProfileReviewSession((current) => {
        if (current !== null) {
          return {
            ...current,
            isOpen: true,
            source: payload.source,
            candidateOptions: payload.candidateOptions,
            helperLogPath: payload.helperLogPath,
            installMessage: payload.message,
            saveError: null,
          };
        }

        return createProfileReviewSessionState(payload);
      });
      setEditorTab('install');
      return true;
    }

    const currentSession = profileReviewSession;
    if (currentSession !== null && isProfileReviewSessionDirty(currentSession)) {
      return requestReviewConfirmation({
        title: 'Replace the current review draft?',
        body: `A newer install result is ready for ${payload.profileName}. Replacing it will discard the unsaved review draft that is open now.`,
        confirmLabel: 'Replace draft',
        cancelLabel: 'Keep current draft',
        tone: 'warning',
        restoreIsOpen: currentSession.isOpen,
        onConfirm: () => {
          setProfileReviewSession(createProfileReviewSessionState(payload));
          setEditorTab('install');
        },
        onCancel: () => undefined,
      });
    }

    setProfileReviewSession(createProfileReviewSessionState(payload));
    setEditorTab('install');
    return true;
  }

  function handleCloseProfileReview() {
    if (profileReviewSession === null) {
      return;
    }

    if (!isProfileReviewSessionDirty(profileReviewSession)) {
      updateProfileReviewSession(setProfileReviewSession, (current) => ({
        ...current,
        isOpen: false,
      }));
      return;
    }

    void requestReviewConfirmation({
      title: 'Hide the review?',
      body: 'Your review draft has unsaved edits. Hide the modal and reopen it later from Verify if you want to continue editing.',
      confirmLabel: 'Hide review',
      cancelLabel: 'Keep editing',
      tone: 'warning',
      restoreIsOpen: profileReviewSession.isOpen,
      onConfirm: () => {
        updateProfileReviewSession(setProfileReviewSession, (current) => ({
          ...current,
          isOpen: false,
        }));
      },
      onCancel: () => undefined,
    });
  }

  function handleProfileReviewNameChange(value: string) {
    updateProfileReviewSession(setProfileReviewSession, (current) => ({
      ...current,
      profileName: value,
      saveError: null,
    }));
  }

  function handleProfileReviewUpdate(updater: (current: GameProfile) => GameProfile) {
    updateProfileReviewSession(setProfileReviewSession, (current) => ({
      ...current,
      draftProfile: updater(current.draftProfile),
      saveError: null,
    }));
  }

  async function handleInstallActionConfirmation(action: 'retry' | 'reset') {
    if (profileReviewSession === null || !isProfileReviewSessionDirty(profileReviewSession)) {
      return true;
    }

    const confirmationText =
      action === 'retry'
        ? 'Starting another install will discard the current review draft before the new result arrives.'
        : 'Resetting the install form will discard the current review draft and clear the install session.';

    return requestReviewConfirmation({
      title: action === 'retry' ? 'Start a new install?' : 'Reset the install session?',
      body: confirmationText,
      confirmLabel: action === 'retry' ? 'Start retry' : 'Reset form',
      cancelLabel: 'Keep current draft',
      tone: 'danger',
      restoreIsOpen: profileReviewSession.isOpen,
      onConfirm: () => {
        setProfileReviewSession(null);
      },
      onCancel: () => undefined,
    });
  }

  async function handleSaveProfileReview() {
    if (profileReviewSession === null) {
      return;
    }

    const profileNameTrimmed = profileReviewSession.profileName.trim();
    const executablePathTrimmed = profileReviewSession.draftProfile.game.executable_path.trim();

    if (!profileNameTrimmed || !executablePathTrimmed) {
      updateProfileReviewSession(setProfileReviewSession, (current) => ({
        ...current,
        saveError: !profileNameTrimmed
          ? 'Profile name is required before saving the review draft.'
          : 'Select the final executable before saving the review draft.',
      }));
      return;
    }

    const { profileName: draftProfileName, draftProfile } = profileReviewSession;
    updateProfileReviewSession(setProfileReviewSession, (current) => ({
      ...current,
      saveError: null,
    }));

    const persistResult = await state.persistProfileDraft(draftProfileName, draftProfile);

    if (!persistResult.ok) {
      updateProfileReviewSession(setProfileReviewSession, (current) => ({
        ...current,
        saveError: persistResult.error,
      }));
      return;
    }

    setProfileReviewSession(null);
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

  const reviewDirty = useMemo(
    () => profileReviewSession !== null && isProfileReviewSessionDirty(profileReviewSession),
    [profileReviewSession],
  );

  const reviewCanSave =
    profileReviewSession !== null &&
    profileReviewSession.profileName.trim().length > 0 &&
    profileReviewSession.draftProfile.game.executable_path.trim().length > 0 &&
    !saving &&
    !deleting &&
    !loading;
  const reviewFinalExecutableMissing =
    profileReviewSession !== null && profileReviewSession.draftProfile.game.executable_path.trim().length === 0;

  let reviewDescription = '';
  let reviewModalStatusTone: 'neutral' | 'success' | 'warning' | 'danger' = 'neutral';
  if (profileReviewSession !== null) {
    if (reviewFinalExecutableMissing) {
      reviewDescription =
        'The review draft is still incomplete. Select the final executable before saving, and the draft will stay open until you finish.';
    } else {
      reviewDescription = `${profileReviewSession.installMessage} Saving persists the profile and returns you to the Profile tab.`.trim();
    }
    if (profileReviewSession.saveError) {
      reviewModalStatusTone = 'danger';
    } else if (reviewFinalExecutableMissing || reviewDirty) {
      reviewModalStatusTone = 'warning';
    } else {
      reviewModalStatusTone = 'neutral';
    }
  }

  return (
    <section className="crosshook-profile-editor-panel">
      <div style={{ display: 'flex', justifyContent: 'space-between', gap: 16, alignItems: 'center' }}>
        <div style={{ display: 'grid', gap: 6 }}>
          <h2 style={{ margin: 0, fontSize: 18 }}>Profile</h2>
          <p className="crosshook-help-text">Select an existing profile or type a new name before saving.</p>
        </div>
        <button type="button" className="crosshook-button crosshook-button--secondary" onClick={() => void refreshProfiles()}>
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
        <div>
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
          <div style={{ display: 'flex', gap: 12, flexWrap: 'wrap', marginTop: 18 }}>
            <button type="button" className="crosshook-button" onClick={() => void saveProfile()} disabled={!canSave}>
              {saving ? 'Saving...' : 'Save'}
            </button>
            <button
              type="button"
              className="crosshook-button crosshook-button--secondary"
              onClick={() => void confirmDelete(profileName)}
              disabled={!canDelete}
            >
              {deleting ? 'Deleting...' : 'Delete'}
            </button>
            <div style={{ display: 'flex', alignItems: 'center', color: dirty ? '#ffd166' : '#9bb1c8' }}>
              {loading ? 'Loading...' : dirty ? 'Unsaved changes' : 'No unsaved changes'}
            </div>
          </div>

          {error ? <div className="crosshook-error-banner crosshook-error-banner--section">{error}</div> : null}
        </div>
      ) : (
        <InstallGamePanel
          onOpenProfileReview={handleOpenProfileReview}
          onRequestInstallAction={handleInstallActionConfirmation}
        />
      )}

      {profileReviewSession !== null && (profileReviewSession.isOpen || reviewConfirmation !== null) ? (
        <ProfileReviewModal
          open={profileReviewSession.isOpen || reviewConfirmation !== null}
          title="Review Generated Profile"
          statusLabel={profileReviewSession.source === 'manual-verify' ? 'Manual verify' : 'Install complete'}
          profileName={profileReviewSession.profileName}
          executablePath={profileReviewSession.draftProfile.game.executable_path}
          prefixPath={profileReviewSession.draftProfile.runtime.prefix_path}
          helperLogPath={profileReviewSession.helperLogPath}
          description={reviewDescription}
          statusTone={reviewModalStatusTone}
          onClose={handleCloseProfileReview}
          confirmation={
            reviewConfirmation
              ? {
                  title: reviewConfirmation.title,
                  body: reviewConfirmation.body,
                  confirmLabel: reviewConfirmation.confirmLabel,
                  cancelLabel: reviewConfirmation.cancelLabel,
                  tone: reviewConfirmation.tone,
                  onConfirm: () => resolveReviewConfirmation(true),
                  onCancel: () => resolveReviewConfirmation(false),
                }
              : null
          }
          footer={
            <div style={{ display: 'flex', gap: 12, flexWrap: 'wrap', justifyContent: 'flex-end' }}>
              <button
                type="button"
                className="crosshook-button crosshook-button--secondary"
                onClick={handleCloseProfileReview}
                disabled={saving}
              >
                Close Review
              </button>
              <button
                type="button"
                className="crosshook-button"
                onClick={() => void handleSaveProfileReview()}
                disabled={!reviewCanSave}
              >
                {saving ? 'Saving...' : 'Save Profile'}
              </button>
            </div>
          }
        >
          <div style={{ display: 'grid', gap: 16 }}>
            {profileReviewSession.saveError ? (
              <div className="crosshook-error-banner">{profileReviewSession.saveError}</div>
            ) : null}

            <ProfileFormSections
              profileName={profileReviewSession.profileName}
              profile={profileReviewSession.draftProfile}
              launchMethod={profileReviewSession.draftProfile.launch.method || 'proton_run'}
              protonInstalls={protonInstalls}
              protonInstallsError={protonInstallsError}
              reviewMode
              onProfileNameChange={handleProfileReviewNameChange}
              onUpdateProfile={handleProfileReviewUpdate}
            />
            {reviewFinalExecutableMissing ? (
              <div className="crosshook-warning-banner">Save is blocked until the final executable is selected.</div>
            ) : null}
          </div>
        </ProfileReviewModal>
      ) : null}

      {pendingDelete && (
        <div className="crosshook-profile-editor-delete-overlay" data-crosshook-focus-root="modal">
          <div className="crosshook-profile-editor-delete-dialog">
            <h3 style={{ margin: '0 0 12px' }}>Delete Profile</h3>
            <p>
              Delete profile <strong>{pendingDelete.name}</strong>?
            </p>
            {pendingDelete.launcherInfo && (
              <div className="crosshook-profile-editor-delete-warning">
                <p style={{ margin: '0 0 6px', fontWeight: 600 }}>Launcher files will also be removed:</p>
                <p style={{ margin: '2px 0', color: '#d1d5db', wordBreak: 'break-all' }}>
                  {pendingDelete.launcherInfo.script_path}
                </p>
                <p style={{ margin: '2px 0', color: '#d1d5db', wordBreak: 'break-all' }}>
                  {pendingDelete.launcherInfo.desktop_entry_path}
                </p>
              </div>
            )}
            <div className="crosshook-profile-editor-delete-actions">
              <button
                type="button"
                className="crosshook-button crosshook-button--secondary"
                onClick={cancelDelete}
                data-crosshook-modal-close
              >
                Cancel
              </button>
              <button type="button" className="crosshook-profile-editor-delete-confirm" onClick={() => void executeDelete()}>
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
    <div className="crosshook-profile-editor-page">
      <div style={{ display: 'grid', gap: 18, maxWidth: 1180, margin: '0 auto' }}>
        <header style={{ display: 'grid', gap: 8 }}>
          <div className="crosshook-profile-editor-eyebrow">CrossHook Native</div>
          <h1 style={{ margin: 0, fontSize: 32, fontWeight: 800 }}>Profile Editor</h1>
          <p className="crosshook-help-text" style={{ maxWidth: 760 }}>
            Edit a profile, save it to Tauri storage, and configure the correct Steam, Proton, or native runner path.
          </p>
        </header>
        <ProfileEditorView state={state} />
      </div>
    </div>
  );
}

export default ProfileEditor;
