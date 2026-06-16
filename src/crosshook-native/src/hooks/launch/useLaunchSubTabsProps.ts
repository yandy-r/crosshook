import { useCallback, useMemo } from 'react';
import type { LaunchSubTabsProps } from '../../components/launch-subtabs/types';
import { useProfileContext } from '../../context/ProfileContext';
import { useProfileHealthContext } from '../../context/ProfileHealthContext';
import { DEFAULT_GAMESCOPE_CONFIG, DEFAULT_MANGOHUD_CONFIG } from '../../types/profile';
import { useLaunchEnvironmentAutosave } from '../profile/useLaunchEnvironmentAutosave';
import { useProtonDbApply } from '../profile/useProtonDbApply';
import { useCommandArgumentCatalog } from '../useCommandArgumentCatalog';
import { useProtonDbSuggestions } from '../useProtonDbSuggestions';

export interface UseLaunchSubTabsPropsInput {
  /**
   * Whether the current gamescope session is already running.
   * Derived by the call site from `useLaunchDepGate` (LaunchPage) or its
   * equivalent in the Hero Detail launch tab.
   */
  isGamescopeRunning: boolean;
  /**
   * Steam App ID resolved from the active profile for cover-art and metadata.
   * Pass the result of `resolveArtAppId(profile)`.
   */
  resolvedSteamAppId: string;
  /**
   * Whether the profile has been saved and is in the profile list.
   * Used to gate the environment autosave path.
   */
  hasSavedSelectedProfile: boolean;
}

/**
 * Assembles the full `LaunchSubTabsProps` payload from the active profile
 * context and the call-site-specific dep-gate values.
 *
 * Both `LaunchPage` and the Hero Detail launch tab can use this hook, each
 * supplying their own derivations of `isGamescopeRunning`,
 * `resolvedSteamAppId`, and `hasSavedSelectedProfile`.
 */
export function useLaunchSubTabsProps({
  isGamescopeRunning,
  resolvedSteamAppId,
  hasSavedSelectedProfile,
}: UseLaunchSubTabsPropsInput): LaunchSubTabsProps {
  const profileState = useProfileContext();
  const { healthByName } = useProfileHealthContext();

  const { profile, selectedProfile, profileName } = profileState;
  const activeCollectionId = profileState.activeCollectionId;

  // ---------------------------------------------------------------------------
  // Derived values from profile
  // ---------------------------------------------------------------------------

  const selectedTrainerVersion =
    selectedProfile.trim().length > 0 ? (healthByName[selectedProfile]?.metadata?.trainer_version ?? null) : null;

  const showProtonDbLookup =
    profileState.launchMethod === 'steam_applaunch' || profileState.launchMethod === 'proton_run';

  const optimizationPresetNames = useMemo(
    () => Object.keys(profile.launch.presets ?? {}).sort((a, b) => a.localeCompare(b)),
    [profile.launch.presets]
  );

  // ---------------------------------------------------------------------------
  // ProtonDB suggestions
  // ---------------------------------------------------------------------------

  const suggestions = useProtonDbSuggestions(resolvedSteamAppId, selectedProfile);
  const { catalog: commandArgumentCatalog } = useCommandArgumentCatalog();

  const handleAcceptSuggestion = useCallback(
    async (request: Parameters<typeof suggestions.acceptSuggestion>[0]): Promise<void> => {
      const result = await suggestions.acceptSuggestion(request);
      if (result.appliedKeys.length > 0 || result.toggledOptionIds.length > 0) {
        void profileState.selectProfile(selectedProfile, {
          collectionId: activeCollectionId ?? undefined,
        });
      }
    },
    [suggestions.acceptSuggestion, profileState.selectProfile, selectedProfile, activeCollectionId]
  );

  // ---------------------------------------------------------------------------
  // ProtonDB apply
  // ---------------------------------------------------------------------------

  const protonDb = useProtonDbApply({
    profile,
    catalog: profileState.catalog,
    onUpdateProfile: profileState.updateProfile,
    onAcceptSuggestion: handleAcceptSuggestion,
  });

  // ---------------------------------------------------------------------------
  // Environment autosave (400 ms blur debounce)
  // ---------------------------------------------------------------------------

  const { handleEnvironmentBlurAutoSave } = useLaunchEnvironmentAutosave({
    hasSavedSelectedProfile,
    profile,
    profileName,
    persistProfileDraft: profileState.persistProfileDraft,
  });

  // ---------------------------------------------------------------------------
  // Gamescope / MangoHud change handlers
  // ---------------------------------------------------------------------------

  const handleGamescopeChange = useCallback(
    (gamescope: Parameters<LaunchSubTabsProps['onGamescopeChange']>[0]) => {
      profileState.updateLaunchSetting((current) => ({
        ...current,
        launch: { ...current.launch, gamescope },
      }));
    },
    [profileState.updateLaunchSetting]
  );

  const handleMangoHudChange = useCallback(
    (mangohud: Parameters<LaunchSubTabsProps['onMangoHudChange']>[0]) => {
      profileState.updateLaunchSetting((current) => ({
        ...current,
        launch: { ...current.launch, mangohud },
      }));
    },
    [profileState.updateLaunchSetting]
  );

  // ---------------------------------------------------------------------------
  // Optimization preset handler
  // ---------------------------------------------------------------------------

  const handleSelectOptimizationPreset = useCallback(
    (name: string) => {
      void profileState.switchLaunchOptimizationPreset(name);
    },
    [profileState.switchLaunchOptimizationPreset]
  );

  const handleApplyBundledPreset = useCallback(
    (presetId: string) => {
      void profileState.applyBundledOptimizationPreset(presetId);
    },
    [profileState.applyBundledOptimizationPreset]
  );

  // ---------------------------------------------------------------------------
  // ProtonDB overwrite handler
  // ---------------------------------------------------------------------------

  const handleConfirmProtonDbOverwrite = useCallback(
    (overwriteKeys: readonly string[]) => {
      if (protonDb.pendingOverwrite) {
        protonDb.applyGroup(protonDb.pendingOverwrite.group, overwriteKeys);
      }
    },
    [protonDb.pendingOverwrite, protonDb.applyGroup]
  );

  const handleUpdateProtonDbResolution = useCallback(
    (key: string, resolution: 'keep_current' | 'use_suggestion') => {
      protonDb.updateOverwriteResolution(
        protonDb.pendingOverwrite == null
          ? null
          : {
              ...protonDb.pendingOverwrite,
              resolutions: { ...protonDb.pendingOverwrite.resolutions, [key]: resolution },
            }
      );
    },
    [protonDb.pendingOverwrite, protonDb.updateOverwriteResolution]
  );

  // ---------------------------------------------------------------------------
  // Assemble and return
  // ---------------------------------------------------------------------------

  return {
    launchMethod: profileState.launchMethod,
    steamAppId: resolvedSteamAppId || undefined,
    customCoverArtPath: profile.game.custom_cover_art_path,

    gamescopeConfig: profile.launch.gamescope ?? DEFAULT_GAMESCOPE_CONFIG,
    onGamescopeChange: handleGamescopeChange,
    isInsideGamescopeSession: isGamescopeRunning,

    mangoHudConfig: profile.launch.mangohud ?? DEFAULT_MANGOHUD_CONFIG,
    onMangoHudChange: handleMangoHudChange,
    showMangoHudOverlayEnabled: profile.launch.optimizations.enabled_option_ids.includes('show_mangohud_overlay'),

    enabledOptionIds: profile.launch.optimizations.enabled_option_ids,
    onToggleOption: profileState.toggleLaunchOptimization,
    launchOptimizationsStatus: profileState.launchOptimizationsStatus,
    optimizationPresetNames,
    activeOptimizationPreset: profile.launch.active_preset ?? '',
    onSelectOptimizationPreset: handleSelectOptimizationPreset,
    bundledOptimizationPresets: profileState.bundledOptimizationPresets,
    onApplyBundledPreset: handleApplyBundledPreset,
    optimizationPresetActionBusy: profileState.optimizationPresetActionBusy,
    onSaveManualPreset: profileState.saveManualOptimizationPreset,
    catalog: profileState.catalog,

    commandArguments: profile.launch.command_arguments,
    onToggleCommandArgument: profileState.toggleCommandArgument,
    onUpdateCommandArgumentsCustomArgs: profileState.updateCommandArgumentsCustomArgs,
    commandArgumentCatalog,
    commandArgumentsAutoSaveStatus: profileState.commandArgumentsAutoSaveStatus,

    customEnvVars: profile.launch.custom_env_vars,
    profileName,
    onUpdateProfile: profileState.updateLaunchSetting,
    onEnvironmentBlurAutoSave: handleEnvironmentBlurAutoSave,

    showProtonDbLookup,
    trainerVersion: selectedTrainerVersion,
    onApplyProtonDbEnvVars: protonDb.applyEnvVars,
    applyingProtonDbGroupId: protonDb.applyingGroupId,
    protonDbStatusMessage: protonDb.statusMessage,
    pendingProtonDbOverwrite: protonDb.pendingOverwrite,
    onConfirmProtonDbOverwrite: handleConfirmProtonDbOverwrite,
    onCancelProtonDbOverwrite: protonDb.clearOverwrite,
    onUpdateProtonDbResolution: handleUpdateProtonDbResolution,

    suggestionSet: suggestions.suggestionSet,
    onAcceptSuggestion: protonDb.acceptSuggestion,
    onDismissSuggestion: suggestions.dismissSuggestion,

    gamescopeAutoSaveStatus: profileState.gamescopeAutoSaveStatus,
    mangoHudAutoSaveStatus: profileState.mangoHudAutoSaveStatus,
  };
}
