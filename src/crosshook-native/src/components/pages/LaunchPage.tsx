import { useCallback, useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

import LaunchPanel from '../LaunchPanel';
import { LaunchSubTabs } from '../LaunchSubTabs';
import { ThemedSelect } from '../ui/ThemedSelect';
import { useProfileContext } from '../../context/ProfileContext';
import { useProfileHealthContext } from '../../context/ProfileHealthContext';
import type { ProtonDbRecommendationGroup } from '../../types/protondb';
import { DEFAULT_GAMESCOPE_CONFIG, DEFAULT_MANGOHUD_CONFIG } from '../../types/profile';
import { buildProfileLaunchRequest } from '../../utils/launch';
import { mergeProtonDbEnvVarGroup, type PendingProtonDbOverwrite } from '../../utils/protondb';

export function LaunchPage() {
  const profileState = useProfileContext();
  const { healthByName } = useProfileHealthContext();
  const profile = profileState.profile;
  const selectedName = profileState.selectedProfile || '';
  const launchRequest = buildProfileLaunchRequest(
    profile,
    profileState.launchMethod,
    profileState.steamClientInstallPath,
    selectedName
  );
  const profileId = profileState.profileName.trim() || selectedName || 'new-profile';
  const [isInsideGamescopeSession, setIsInsideGamescopeSession] = useState(false);
  useEffect(() => {
    invoke<boolean>('check_gamescope_session')
      .then(setIsInsideGamescopeSession)
      .catch(() => {});
  }, []);

  const pinnedSet = useMemo(() => new Set(profileState.favoriteProfiles), [profileState.favoriteProfiles]);
  const handleTogglePin = useCallback(
    (value: string) => {
      void profileState.toggleFavorite(value, !pinnedSet.has(value));
    },
    [pinnedSet, profileState.toggleFavorite]
  );

  const optimizationPresetNames = useMemo(
    () => Object.keys(profile.launch.presets ?? {}).sort((a, b) => a.localeCompare(b)),
    [profile.launch.presets]
  );
  const hasSavedSelectedProfile = useMemo(() => {
    const trimmedProfileName = profileState.profileName.trim();
    const trimmedSelectedProfile = profileState.selectedProfile.trim();
    return (
      trimmedProfileName.length > 0 &&
      trimmedSelectedProfile.length > 0 &&
      trimmedProfileName === trimmedSelectedProfile &&
      profileState.profiles.includes(trimmedProfileName)
    );
  }, [profileState.profileName, profileState.profiles, profileState.selectedProfile]);

  const selectedTrainerVersion =
    profileState.selectedProfile.trim().length > 0
      ? (healthByName[profileState.selectedProfile]?.metadata?.trainer_version ?? null)
      : null;

  const showProtonDbLookup =
    profileState.launchMethod === 'steam_applaunch' || profileState.launchMethod === 'proton_run';

  const [pendingProtonDbOverwrite, setPendingProtonDbOverwrite] = useState<PendingProtonDbOverwrite | null>(null);
  const [applyingProtonDbGroupId, setApplyingProtonDbGroupId] = useState<string | null>(null);
  const [protonDbStatusMessage, setProtonDbStatusMessage] = useState<string | null>(null);

  useEffect(() => {
    setPendingProtonDbOverwrite(null);
    setApplyingProtonDbGroupId(null);
    setProtonDbStatusMessage(null);
  }, [profileState.profileName, profile.steam.app_id, profileState.launchMethod]);

  const applyProtonDbGroup = useCallback(
    (group: ProtonDbRecommendationGroup, overwriteKeys: readonly string[]) => {
      const merge = {
        appliedKeys: [] as string[],
        unchangedKeys: [] as string[],
      };
      profileState.updateProfile((current) => {
        const nextMerge = mergeProtonDbEnvVarGroup(current.launch.custom_env_vars, group, overwriteKeys);
        merge.appliedKeys = nextMerge.appliedKeys;
        merge.unchangedKeys = nextMerge.unchangedKeys;
        return {
          ...current,
          launch: {
            ...current.launch,
            custom_env_vars: nextMerge.mergedEnvVars,
          },
        };
      });
      setApplyingProtonDbGroupId(null);
      setPendingProtonDbOverwrite(null);

      const appliedCount = merge.appliedKeys.length;
      const unchangedCount = merge.unchangedKeys.length;
      if (appliedCount > 0) {
        setProtonDbStatusMessage(
          `Applied ${appliedCount} ProtonDB environment variable${appliedCount === 1 ? '' : 's'}${
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
    [profileState.updateProfile]
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

  const handleEnvironmentBlurAutoSave = useCallback(
    (trigger: 'key' | 'value', row: Readonly<{ key: string; value: string }>, nextEnvVars: Readonly<Record<string, string>>) => {
      if (!hasSavedSelectedProfile) {
        return;
      }
      if (trigger === 'value' && row.key.trim().length === 0) {
        return;
      }
      setTimeout(() => {
        void profileState.persistProfileDraft(profileState.profileName, {
          ...profileState.profile,
          launch: {
            ...profileState.profile.launch,
            custom_env_vars: { ...nextEnvVars },
          },
        });
      }, 0);
    },
    [hasSavedSelectedProfile, profileState.persistProfileDraft, profileState.profile, profileState.profileName]
  );

  return (
    <div className="crosshook-page-scroll-shell crosshook-page-scroll-shell--fill crosshook-page-scroll-shell--launch">
      <div className="crosshook-route-stack crosshook-launch-page__grid">
        <LaunchPanel
          profileId={profileId}
          method={profileState.launchMethod}
          request={launchRequest}
          profileSelectSlot={
            <ThemedSelect
              id="launch-profile-selector"
              value={profileState.selectedProfile}
              onValueChange={(name) => void profileState.selectProfile(name)}
              placeholder="Select a profile"
              pinnedValues={pinnedSet}
              onTogglePin={handleTogglePin}
              ariaLabelledby="launch-active-profile-label"
              options={profileState.profiles.map((name) => ({ value: name, label: name }))}
            />
          }
          tabsSlot={
            <LaunchSubTabs
              launchMethod={profileState.launchMethod}
              steamAppId={profile.steam.app_id}
              customCoverArtPath={profile.game.custom_cover_art_path}
              gamescopeConfig={profile.launch.gamescope ?? DEFAULT_GAMESCOPE_CONFIG}
              onGamescopeChange={(gamescope) => {
                profileState.updateLaunchSetting((current) => ({
                  ...current,
                  launch: { ...current.launch, gamescope },
                }));
              }}
              isInsideGamescopeSession={isInsideGamescopeSession}
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
              gamescopeAutoSaveStatus={profileState.gamescopeAutoSaveStatus}
              mangoHudAutoSaveStatus={profileState.mangoHudAutoSaveStatus}
            />
          }
        />
      </div>
    </div>
  );
}

export default LaunchPage;
