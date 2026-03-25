import { useEffect, useRef, useState, type CSSProperties } from 'react';
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

const panelStyle: CSSProperties = {
  background: 'rgba(13, 20, 31, 0.92)',
  border: '1px solid rgba(120, 145, 177, 0.2)',
  borderRadius: 18,
  boxShadow: '0 24px 60px rgba(0, 0, 0, 0.35)',
  padding: 20,
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

type ReviewConfirmationState = ProfileReviewModalConfirmation & {
  restoreIsOpen: boolean;
};

function createProfileReviewSessionState(payload: InstallProfileReviewPayload): ProfileReviewSession {
  return {
    isOpen: true,
    source: payload.source,
    profileName: payload.profileName,
    originalProfile: payload.generatedProfile,
    draftProfile: payload.generatedProfile,
    candidateOptions: payload.candidateOptions,
    helperLogPath: payload.helperLogPath,
    installMessage: payload.message,
    dirty: false,
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
      setProfileReviewSession((current) => {
        if (current === null) {
          return current;
        }

        return {
          ...current,
          isOpen: confirmation.restoreIsOpen,
        };
      });
      confirmation.onCancel();
    }

    resolver?.(confirmed);
  }

  function requestReviewConfirmation(confirmation: ReviewConfirmationState) {
    if (reviewConfirmationResolverRef.current !== null) {
      return Promise.resolve(false);
    }

    setEditorTab('install');
    setProfileReviewSession((current) => {
      if (current === null) {
        return current;
      }

      return {
        ...current,
        isOpen: true,
      };
    });

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
        if (currentSession.dirty) {
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
              setProfileReviewSession((current) => {
                if (current === null) {
                  return current;
                }

                return {
                  ...current,
                  isOpen: true,
                };
              });
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
    if (currentSession?.dirty) {
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

    if (!profileReviewSession.dirty) {
      setProfileReviewSession((current) => {
        if (current === null) {
          return current;
        }

        return {
          ...current,
          isOpen: false,
        };
      });
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
        setProfileReviewSession((current) => {
          if (current === null) {
            return current;
          }

          return {
            ...current,
            isOpen: false,
          };
        });
      },
      onCancel: () => undefined,
    });
  }

  function handleProfileReviewNameChange(value: string) {
    setProfileReviewSession((current) => {
      if (current === null) {
        return current;
      }

      return {
        ...current,
        profileName: value,
        dirty: true,
        saveError: null,
      };
    });
  }

  function handleProfileReviewUpdate(updater: (current: GameProfile) => GameProfile) {
    setProfileReviewSession((current) => {
      if (current === null) {
        return current;
      }

      return {
        ...current,
        draftProfile: updater(current.draftProfile),
        dirty: true,
        saveError: null,
      };
    });
  }

  async function handleInstallActionConfirmation(action: 'retry' | 'reset') {
    if (profileReviewSession === null || !profileReviewSession.dirty) {
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
      setProfileReviewSession((current) => {
        if (current === null) {
          return current;
        }

        return {
          ...current,
          saveError: !profileNameTrimmed
            ? 'Profile name is required before saving the review draft.'
            : 'Select the final executable before saving the review draft.',
        };
      });
      return;
    }

    const { profileName: draftProfileName, draftProfile } = profileReviewSession;
    setProfileReviewSession((current) => {
      if (current === null) {
        return current;
      }

      return {
        ...current,
        saveError: null,
      };
    });

    const persistResult = await state.persistProfileDraft(draftProfileName, draftProfile);

    if (!persistResult.ok) {
      setProfileReviewSession((current) => {
        if (current === null) {
          return current;
        }

        return {
          ...current,
          saveError: persistResult.error,
        };
      });
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

  const reviewCanSave =
    profileReviewSession !== null &&
    profileReviewSession.profileName.trim().length > 0 &&
    profileReviewSession.draftProfile.game.executable_path.trim().length > 0 &&
    !saving &&
    !deleting &&
    !loading;
  const reviewFinalExecutableMissing =
    profileReviewSession !== null && profileReviewSession.draftProfile.game.executable_path.trim().length === 0;
  const reviewDescription =
    profileReviewSession === null
      ? ''
      : reviewFinalExecutableMissing
        ? 'The review draft is still incomplete. Select the final executable before saving, and the draft will stay open until you finish.'
        : `${profileReviewSession.installMessage} Saving persists the profile and returns you to the Profile tab.`.trim();

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
        <div>
          <ProfileFormSections
            profileName={profileName}
            profiles={profiles}
            selectedProfile={selectedProfile}
            profile={profile}
            launchMethod={launchMethod}
            protonInstalls={protonInstalls}
            protonInstallsError={protonInstallsError}
            onProfileNameChange={setProfileName}
            onSelectProfile={selectProfile}
            onUpdateProfile={updateProfile}
          />
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
          statusTone={
            profileReviewSession.saveError
              ? 'danger'
              : reviewFinalExecutableMissing || profileReviewSession.dirty
                ? 'warning'
                : 'neutral'
          }
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
              <div
                style={{
                  borderRadius: 12,
                  padding: 12,
                  background: 'rgba(140, 40, 40, 0.2)',
                  border: '1px solid rgba(255, 90, 90, 0.3)',
                  color: '#ffd4d4',
                }}
              >
                {profileReviewSession.saveError}
              </div>
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
              <div
                style={{
                  borderRadius: 12,
                  padding: 12,
                  background: 'rgba(245, 158, 11, 0.12)',
                  border: '1px solid rgba(245, 158, 11, 0.26)',
                  color: '#fcd34d',
                }}
              >
                Save is blocked until the final executable is selected.
              </div>
            ) : null}
          </div>
        </ProfileReviewModal>
      ) : null}

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
