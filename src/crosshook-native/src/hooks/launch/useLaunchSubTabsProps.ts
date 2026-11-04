import { useCallback, useMemo, useRef } from 'react';
import type { LaunchSubTabsProps } from '../../components/launch-subtabs/types';
import { useProfileContext } from '../../context/ProfileContext';
import { useProfileHealthContext } from '../../context/ProfileHealthContext';
import { DEFAULT_GAMESCOPE_CONFIG, DEFAULT_MANGOHUD_CONFIG } from '../../types/profile';
import type { SuggestionAcceptance } from '../../types/protondb';
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
  // Latest selection and dirty flag, read after the accept IPC resolves so a
  // reload never targets a profile the user has already navigated away from
  // and never discards edits made while the accept was in flight.
  const selectedProfileRef = useRef(selectedProfile);
  selectedProfileRef.current = selectedProfile;
  const dirtyRef = useRef(profileState.dirty);
  dirtyRef.current = profileState.dirty;

  const sectionAutosavePending = [
    profileState.launchOptimizationsStatus,
    profileState.gamescopeAutoSaveStatus,
    profileState.trainerGamescopeAutoSaveStatus,
    profileState.mangoHudAutoSaveStatus,
    profileState.commandArgumentsAutoSaveStatus,
  ].some((status) => status.tone === 'saving');

  const handleAcceptSuggestion = useCallback(
    async (acceptance: SuggestionAcceptance): Promise<void> => {
      // Accepting writes the profile on disk and then reloads it. The reload
      // resets the section autosave baselines, so a debounced section save
      // that has not landed yet would be silently dropped.
      if (sectionAutosavePending) {
        throw new Error('Launch settings are still saving. Try again in a moment.');
      }
      const requestedProfile = selectedProfile;
      // Save unsaved draft edits first so the post-accept reload cannot
      // discard them (mirrors the Profiles tab's flush-before-switch).
      if (profileState.dirty) {
        if (!hasSavedSelectedProfile) {
          throw new Error('Save the profile before accepting ProtonDB suggestions.');
        }
        const saved = await profileState.persistProfileDraft(requestedProfile, profileState.profile);
        if (!saved.ok) {
          throw new Error(saved.error);
        }
        // A successful save reloads the profile, which clears `dirty`; later
        // renders refresh this ref from the real state.
        dirtyRef.current = false;
      }
      const result = await suggestions.acceptSuggestion(acceptance);
      if (selectedProfileRef.current !== requestedProfile || dirtyRef.current) {
        return;
      }
      if (result.appliedKeys.length > 0 || result.toggledOptionIds.length > 0) {
        void profileState.selectProfile(requestedProfile, {
          collectionId: activeCollectionId ?? undefined,
        });
      }
    },
    [
      sectionAutosavePending,
      suggestions.acceptSuggestion,
      profileState.dirty,
      profileState.persistProfileDraft,
      profileState.profile,
      profileState.selectProfile,
      hasSavedSelectedProfile,
      selectedProfile,
      activeCollectionId,
    ]
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
