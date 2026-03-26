import { useEffect, useMemo, useRef, useState, type Dispatch, type SetStateAction } from 'react';
import { invoke } from '@tauri-apps/api/core';

import InstallGamePanel from '../InstallGamePanel';
import ProfileFormSections, { type ProtonInstallOption } from '../ProfileFormSections';
import ProfileReviewModal, { type ProfileReviewModalConfirmation } from '../ProfileReviewModal';
import { usePreferencesContext } from '../../context/PreferencesContext';
import { useProfileContext } from '../../context/ProfileContext';
import type { GameProfile } from '../../types';
import type { InstallProfileReviewPayload } from '../../types/install';
import type { ProfileReviewSession } from '../../types/profile-review';
import { profilesEqual } from '../../utils/profile-compare';
import { PageBanner, InstallArt } from '../layout/PageBanner';
import type { AppRoute } from '../layout/Sidebar';

type ReviewConfirmationState = ProfileReviewModalConfirmation & {
  restoreIsOpen: boolean;
};

export interface InstallPageProps {
  onNavigate?: (route: AppRoute) => void;
}

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

export function InstallPage({ onNavigate }: InstallPageProps) {
  const { defaultSteamClientInstallPath } = usePreferencesContext();
  const {
    deleting,
    loading,
    persistProfileDraft,
    saving,
    steamClientInstallPath,
  } = useProfileContext();

  const effectiveSteamClientInstallPath = useMemo(
    () => defaultSteamClientInstallPath || steamClientInstallPath,
    [defaultSteamClientInstallPath, steamClientInstallPath],
  );

  const [protonInstalls, setProtonInstalls] = useState<ProtonInstallOption[]>([]);
  const [protonInstallsError, setProtonInstallsError] = useState<string | null>(null);
  const [profileReviewSession, setProfileReviewSession] = useState<ProfileReviewSession | null>(null);
  const [reviewConfirmation, setReviewConfirmation] = useState<ReviewConfirmationState | null>(null);
  const reviewConfirmationResolverRef = useRef<((confirmed: boolean) => void) | null>(null);

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
        },
        onCancel: () => undefined,
      });
    }

    setProfileReviewSession(createProfileReviewSessionState(payload));
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

    const { profileName, draftProfile } = profileReviewSession;
    updateProfileReviewSession(setProfileReviewSession, (current) => ({
      ...current,
      saveError: null,
    }));

    const persistResult = await persistProfileDraft(profileName, draftProfile);

    if (!persistResult.ok) {
      updateProfileReviewSession(setProfileReviewSession, (current) => ({
        ...current,
        saveError: persistResult.error,
      }));
      return;
    }

    setProfileReviewSession(null);
    onNavigate?.('profiles');
  }

  useEffect(() => {
    let active = true;

    async function loadProtonInstalls() {
      try {
        const installs = await invoke<ProtonInstallOption[]>('list_proton_installs', {
          steamClientInstallPath: effectiveSteamClientInstallPath.trim().length > 0 ? effectiveSteamClientInstallPath : undefined,
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
  }, [effectiveSteamClientInstallPath]);

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
      reviewDescription = `${profileReviewSession.installMessage} Saving persists the profile and returns you to the Profiles view.`.trim();
    }

    if (profileReviewSession.saveError) {
      reviewModalStatusTone = 'danger';
    } else if (reviewFinalExecutableMissing || reviewDirty) {
      reviewModalStatusTone = 'warning';
    }
  }

  return (
    <div className="crosshook-content-area">
      <PageBanner
        eyebrow="Setup"
        title="Install game"
        copy="Run the installer in a Proton-backed flow, review the generated profile in-place, and save directly back into the Profiles view when the draft is ready."
        illustration={<InstallArt />}
      />

      <InstallGamePanel
        onOpenProfileReview={handleOpenProfileReview}
        onRequestInstallAction={handleInstallActionConfirmation}
      />

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
                {saving ? 'Saving...' : 'Save and Open Profiles'}
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
              <div className="crosshook-warning-banner">
                Save is blocked until the final executable is selected.
              </div>
            ) : null}
          </div>
        </ProfileReviewModal>
      ) : null}
    </div>
  );
}

export default InstallPage;
