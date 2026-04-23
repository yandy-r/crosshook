import { useId } from 'react';
import { useProtonDbSuggestions } from '../hooks/useProtonDbSuggestions';
import { useProtonDbApply } from '../hooks/profile/useProtonDbApply';
import type { GameProfile, LaunchMethod } from '../types';
import type { AcceptSuggestionRequest } from '../types/protondb';
import type { VersionCorrelationStatus } from '../types/version';
import { resolveArtAppId } from '../utils/art';
import type { OptimizationCatalogPayload } from '../utils/optimization-catalog';
import { CustomEnvironmentVariablesSection } from './CustomEnvironmentVariablesSection';
import ProtonDbLookupCard from './ProtonDbLookupCard';
import ProtonDbOverwriteConfirmation from './ProtonDbOverwriteConfirmation';
import { GameSection } from './profile-sections/GameSection';
import { ProfileIdentitySection } from './profile-sections/ProfileIdentitySection';
import { RunnerMethodSection } from './profile-sections/RunnerMethodSection';
import { RuntimeSection } from './profile-sections/RuntimeSection';
import { TrainerSection } from './profile-sections/TrainerSection';
import { FieldRow } from './profile-form/FormFieldRow';
import { OptionalSection } from './profile-form/OptionalSection';
import { ProtonPathField } from './profile-form/ProtonPathField';
import { LauncherMetadataFields } from './profile-form/LauncherMetadataFields';
import {
  ProfileSelectorField,
  type ProfileFormSectionsProfileSelector,
} from './profile-form/ProfileSelectorField';
import { TrainerVersionSetField } from './profile-form/TrainerVersionSetField';

export type { ProfileFormSectionsProfileSelector };

export type { PendingProtonDbOverwrite } from '../utils/protondb';
export type { ProtonInstallOption } from '../types/proton';
export { formatProtonInstallLabel } from '../utils/proton';
export { deriveSteamClientInstallPath } from '../utils/steam';
export { parentDirectory, updateGameExecutablePath } from './profile-form/helpers';
export { FieldRow, OptionalSection, ProtonPathField, LauncherMetadataFields, TrainerVersionSetField };

type ProfileFormSectionsBaseProps = {
  profileName: string;
  profile: GameProfile;
  launchMethod: LaunchMethod;
  protonInstalls: import('../types/proton').ProtonInstallOption[];
  protonInstallsError: string | null;
  reviewMode?: boolean;
  profileExists?: boolean;
  trainerVersion?: string | null;
  versionStatus?: VersionCorrelationStatus | null;
  onVersionSet?: () => void;
  onProfileNameChange: (value: string) => void;
  onUpdateProfile: (updater: (current: GameProfile) => GameProfile) => void;
  catalog?: OptimizationCatalogPayload | null;
};

export type ProfileFormSectionsProps =
  | (ProfileFormSectionsBaseProps & { profileSelector: ProfileFormSectionsProfileSelector })
  | (ProfileFormSectionsBaseProps & { profileSelector?: undefined });

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
    catalog = null,
  } = props;
  const profileSelector = 'profileSelector' in props ? props.profileSelector : undefined;
  const profileNamesListId = useId();
  const resolvedAppId = resolveArtAppId(profile);
  const suggestions = useProtonDbSuggestions(resolvedAppId, profileName);

  const handleAcceptSuggestion = async (request: AcceptSuggestionRequest): Promise<void> => {
    const result = await suggestions.acceptSuggestion(request);
    onUpdateProfile(() => result.updatedProfile);
  };

  const {
    pendingOverwrite,
    applyingGroupId,
    statusMessage: protonDbStatusMessage,
    applyEnvVars: handleApplyProtonDbEnvVars,
    applyGroup: applyProtonDbGroup,
    updateOverwriteResolution,
    clearOverwrite,
  } = useProtonDbApply({
    profileName,
    profile,
    catalog,
    onUpdateProfile,
    onAcceptSuggestion: handleAcceptSuggestion,
  });

  const profiles = profileSelector?.profiles;
  const selectedProfile = profileSelector?.selectedProfile;
  const showProtonDbLookup = launchMethod === 'steam_applaunch' || launchMethod === 'proton_run';
  const reviewModeNote = reviewMode ? (
    <p className="crosshook-help-text">
      Review mode keeps launch-critical fields expanded and collapses only empty optional overrides.
    </p>
  ) : null;

  const protonDbPanel = showProtonDbLookup ? (
    <div className="crosshook-protondb-panel">
      <ProtonDbLookupCard
        appId={resolvedAppId}
        trainerVersion={trainerVersion}
        versionContext={{ version_status: versionStatus }}
        onApplyEnvVars={reviewMode ? undefined : handleApplyProtonDbEnvVars}
        applyingGroupId={applyingGroupId}
        suggestionSet={reviewMode ? undefined : suggestions.suggestionSet}
        onAcceptSuggestion={reviewMode ? undefined : handleAcceptSuggestion}
        onDismissSuggestion={reviewMode ? undefined : suggestions.dismissSuggestion}
      />

      {protonDbStatusMessage ? (
        <p className="crosshook-help-text" role="status">
          {protonDbStatusMessage}
        </p>
      ) : null}

      {pendingOverwrite ? (
        <ProtonDbOverwriteConfirmation
          pendingProtonDbOverwrite={pendingOverwrite}
          onUpdateProtonDbResolution={(key, choice) =>
            updateOverwriteResolution(
              pendingOverwrite == null
                ? pendingOverwrite
                : {
                    ...pendingOverwrite,
                    resolutions: {
                      ...pendingOverwrite.resolutions,
                      [key]: choice,
                    },
                  }
            )
          }
          onCancelProtonDbOverwrite={clearOverwrite}
          onConfirmProtonDbOverwrite={(selectedKeys) => applyProtonDbGroup(pendingOverwrite.group, selectedKeys)}
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

      <RunnerMethodSection profile={profile} onUpdateProfile={onUpdateProfile} reviewMode={reviewMode} />

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
