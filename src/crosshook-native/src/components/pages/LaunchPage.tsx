import { useCallback, useEffect, useState } from 'react';
import { useProfileHealthContext } from '../../context/ProfileHealthContext';
import { useProtonDbSuggestions } from '../../hooks/useProtonDbSuggestions';
import { useLaunchEnvironmentAutosave } from '../../hooks/profile/useLaunchEnvironmentAutosave';
import type { AcceptSuggestionRequest, ProtonDbRecommendationGroup } from '../../types/protondb';
import { DEFAULT_GAMESCOPE_CONFIG, DEFAULT_MANGOHUD_CONFIG } from '../../types/profile';
import { resolveArtAppId } from '../../utils/art';
import {
  applyProtonDbGroupToProfile,
  mergeProtonDbEnvVarGroup,
  type PendingProtonDbOverwrite,
} from '../../utils/protondb';
import LaunchPanel from '../LaunchPanel';
import { LaunchSubTabs } from '../LaunchSubTabs';
import { RouteBanner } from '../layout/RouteBanner';
import { LaunchDepGateModal } from './launch/LaunchDepGateModal';
import { LaunchProfileSelector } from './launch/LaunchProfileSelector';
import { useLaunchDepGate } from './launch/useLaunchDepGate';
import { useLaunchPageState } from './launch/useLaunchPageState';

export function LaunchPage() {
  const {
    activeCollection,
    activeCollectionId,
    filteredProfiles,
    hasSavedSelectedProfile,
    handleTogglePin,
    launchRequest,
    optimizationPresetNames,
    pinnedSet,
    profile,
    profileId,
    profileState,
    resolvedSteamAppId,
    selectedName,
    setActiveCollectionId,
    settings,
    showNetworkIsolationBadge,
  } = useLaunchPageState();

  const { healthByName } = useProfileHealthContext();

  const selectedTrainerVersion =
    profileState.selectedProfile.trim().length > 0
      ? (healthByName[profileState.selectedProfile]?.metadata?.trainer_version ?? null)
      : null;

  const showProtonDbLookup =
    profileState.launchMethod === 'steam_applaunch' || profileState.launchMethod === 'proton_run';

  const [pendingProtonDbOverwrite, setPendingProtonDbOverwrite] = useState<PendingProtonDbOverwrite | null>(null);
  const [applyingProtonDbGroupId, setApplyingProtonDbGroupId] = useState<string | null>(null);
  const [protonDbStatusMessage, setProtonDbStatusMessage] = useState<string | null>(null);
  const suggestions = useProtonDbSuggestions(resolvedSteamAppId, profileState.selectedProfile);

  const handleAcceptSuggestion = useCallback(
    async (request: AcceptSuggestionRequest): Promise<void> => {
      const result = await suggestions.acceptSuggestion(request);
      if (result.appliedKeys.length > 0 || result.toggledOptionIds.length > 0) {
        // LaunchPage: reload reflects active collection context if any.
        void profileState.selectProfile(selectedName, {
          collectionId: activeCollectionId ?? undefined,
        });
      }
    },
    [suggestions.acceptSuggestion, profileState.selectProfile, selectedName, activeCollectionId]
  );

  const depGate = useLaunchDepGate({
    profile,
    selectedName,
    autoInstallPrefixDeps: settings.auto_install_prefix_deps,
  });

  const { handleEnvironmentBlurAutoSave } = useLaunchEnvironmentAutosave({
    hasSavedSelectedProfile,
    profile,
    profileName: profileState.profileName,
    persistProfileDraft: profileState.persistProfileDraft,
  });

  useEffect(() => {
    setPendingProtonDbOverwrite(null);
    setApplyingProtonDbGroupId(null);
    setProtonDbStatusMessage(null);
  }, []);

  const applyProtonDbGroup = useCallback(
    (group: ProtonDbRecommendationGroup, overwriteKeys: readonly string[]) => {
      const result = { appliedKeys: [] as string[], unchangedKeys: [] as string[], toggledOptionIds: [] as string[] };
      profileState.updateProfile((current) => {
        const applyResult = applyProtonDbGroupToProfile(current, group, overwriteKeys, profileState.catalog);
        result.appliedKeys = applyResult.appliedKeys;
        result.unchangedKeys = applyResult.unchangedKeys;
        result.toggledOptionIds = applyResult.toggledOptionIds;
        return applyResult.nextProfile;
      });
      setApplyingProtonDbGroupId(null);
      setPendingProtonDbOverwrite(null);

      const appliedCount = result.appliedKeys.length;
      const unchangedCount = result.unchangedKeys.length;
      const toggledCount = result.toggledOptionIds.length;
      if (appliedCount > 0 || toggledCount > 0) {
        const parts: string[] = [];
        if (toggledCount > 0) parts.push(`${toggledCount} optimization${toggledCount === 1 ? '' : 's'}`);
        if (appliedCount - toggledCount > 0) {
          const envCount = appliedCount - toggledCount;
          parts.push(`${envCount} env var${envCount === 1 ? '' : 's'}`);
        }
        setProtonDbStatusMessage(
          `Applied ${parts.join(' and ')}${
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
    },
    [profileState.updateProfile, profileState.catalog]
  );

  const handleApplyProtonDbEnvVars = useCallback(
    (group: ProtonDbRecommendationGroup) => {
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
    },
    [applyProtonDbGroup, profile.launch.custom_env_vars]
  );

  return (
    <div className="crosshook-page-scroll-shell crosshook-page-scroll-shell--fill crosshook-page-scroll-shell--launch">
      <div className="crosshook-route-stack crosshook-launch-page__grid">
        <RouteBanner route="launch" />
        <LaunchPanel
          profileId={profileId}
          method={profileState.launchMethod}
          request={launchRequest}
          profile={profile}
          profileSelectSlot={
            <LaunchProfileSelector
              activeCollection={activeCollection}
              activeCollectionId={activeCollectionId}
              filteredProfiles={filteredProfiles}
              pinnedSet={pinnedSet}
              selectedProfile={profileState.selectedProfile}
              showNetworkIsolationBadge={showNetworkIsolationBadge}
              onClearCollectionFilter={() => setActiveCollectionId(null)}
              onSelectProfile={(name) =>
                void profileState.selectProfile(name, {
                  collectionId: activeCollectionId ?? undefined,
                })
              }
              onTogglePin={handleTogglePin}
            />
          }
          tabsSlot={
            <LaunchSubTabs
              launchMethod={profileState.launchMethod}
              steamAppId={resolvedSteamAppId}
              customCoverArtPath={profile.game.custom_cover_art_path}
              gamescopeConfig={profile.launch.gamescope ?? DEFAULT_GAMESCOPE_CONFIG}
              onGamescopeChange={(gamescope) => {
                profileState.updateLaunchSetting((current) => ({
                  ...current,
                  launch: { ...current.launch, gamescope },
                }));
              }}
              isInsideGamescopeSession={depGate.isGamescopeRunning}
              mangoHudConfig={profile.launch.mangohud ?? DEFAULT_MANGOHUD_CONFIG}
              onMangoHudChange={(mangohud) => {
                profileState.updateLaunchSetting((current) => ({
                  ...current,
                  launch: { ...current.launch, mangohud },
                }));
              }}
              showMangoHudOverlayEnabled={profile.launch.optimizations.enabled_option_ids.includes(
                'show_mangohud_overlay'
              )}
              enabledOptionIds={profile.launch.optimizations.enabled_option_ids}
              onToggleOption={profileState.toggleLaunchOptimization}
              launchOptimizationsStatus={profileState.launchOptimizationsStatus}
              optimizationPresetNames={optimizationPresetNames}
              activeOptimizationPreset={profile.launch.active_preset ?? ''}
              onSelectOptimizationPreset={(name) => {
                void profileState.switchLaunchOptimizationPreset(name);
              }}
              bundledOptimizationPresets={profileState.bundledOptimizationPresets}
              onApplyBundledPreset={(presetId) => {
                void profileState.applyBundledOptimizationPreset(presetId);
              }}
              optimizationPresetActionBusy={profileState.optimizationPresetActionBusy}
              onSaveManualPreset={profileState.saveManualOptimizationPreset}
              catalog={profileState.catalog}
              customEnvVars={profile.launch.custom_env_vars}
              profileName={profileState.profileName}
              onUpdateProfile={profileState.updateLaunchSetting}
              onEnvironmentBlurAutoSave={handleEnvironmentBlurAutoSave}
              showProtonDbLookup={showProtonDbLookup}
              trainerVersion={selectedTrainerVersion}
              onApplyProtonDbEnvVars={handleApplyProtonDbEnvVars}
              applyingProtonDbGroupId={applyingProtonDbGroupId}
              protonDbStatusMessage={protonDbStatusMessage}
              pendingProtonDbOverwrite={pendingProtonDbOverwrite}
              onConfirmProtonDbOverwrite={(overwriteKeys) => {
                if (pendingProtonDbOverwrite) {
                  applyProtonDbGroup(pendingProtonDbOverwrite.group, overwriteKeys);
                }
              }}
              onCancelProtonDbOverwrite={() => setPendingProtonDbOverwrite(null)}
              onUpdateProtonDbResolution={(key, resolution) =>
                setPendingProtonDbOverwrite((current) =>
                  current == null ? current : { ...current, resolutions: { ...current.resolutions, [key]: resolution } }
                )
              }
              suggestionSet={suggestions.suggestionSet}
              onAcceptSuggestion={handleAcceptSuggestion}
              onDismissSuggestion={suggestions.dismissSuggestion}
              gamescopeAutoSaveStatus={profileState.gamescopeAutoSaveStatus}
              mangoHudAutoSaveStatus={profileState.mangoHudAutoSaveStatus}
            />
          }
          onBeforeLaunch={depGate.handleBeforeLaunch}
        />
      </div>

      <LaunchDepGateModal depGate={depGate} profile={profile} selectedName={selectedName} />
    </div>
  );
}

export default LaunchPage;
