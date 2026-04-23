import { useCallback } from 'react';
import { useProfileHealthContext } from '../../context/ProfileHealthContext';
import { useLaunchEnvironmentAutosave } from '../../hooks/profile/useLaunchEnvironmentAutosave';
import { useProtonDbApply } from '../../hooks/profile/useProtonDbApply';
import { useProtonDbSuggestions } from '../../hooks/useProtonDbSuggestions';
import { DEFAULT_GAMESCOPE_CONFIG, DEFAULT_MANGOHUD_CONFIG } from '../../types/profile';
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
    effectiveSteamClientInstallPath,
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

  const suggestions = useProtonDbSuggestions(resolvedSteamAppId, profileState.selectedProfile);

  const handleAcceptSuggestion = useCallback(
    async (request: Parameters<typeof suggestions.acceptSuggestion>[0]): Promise<void> => {
      const result = await suggestions.acceptSuggestion(request);
      if (result.appliedKeys.length > 0 || result.toggledOptionIds.length > 0) {
        void profileState.selectProfile(selectedName, {
          collectionId: activeCollectionId ?? undefined,
        });
      }
    },
    [suggestions.acceptSuggestion, profileState.selectProfile, selectedName, activeCollectionId]
  );

  const protonDb = useProtonDbApply({
    profile,
    catalog: profileState.catalog,
    onUpdateProfile: profileState.updateProfile,
    onAcceptSuggestion: handleAcceptSuggestion,
  });

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

  return (
    <div className="crosshook-page-scroll-shell crosshook-page-scroll-shell--fill crosshook-page-scroll-shell--launch">
      <div className="crosshook-route-stack crosshook-launch-page__grid">
        <RouteBanner route="launch" />
        <LaunchPanel
          profileId={profileId}
          method={profileState.launchMethod}
          request={launchRequest}
          profile={profile}
          infoSlot={
            <dl className="crosshook-dashboard-kv-list">
              <div className="crosshook-dashboard-kv-row">
                <dt className="crosshook-dashboard-kv-row__label">Selected profile</dt>
                <dd className="crosshook-dashboard-kv-row__value">
                  {selectedName.trim() !== '' ? selectedName : <span className="crosshook-muted">None selected</span>}
                </dd>
              </div>
              {effectiveSteamClientInstallPath ? (
                <div className="crosshook-dashboard-kv-row">
                  <dt className="crosshook-dashboard-kv-row__label">Steam path</dt>
                  <dd
                    className="crosshook-dashboard-kv-row__value"
                    style={{ fontFamily: 'var(--crosshook-font-mono)', fontSize: '0.85rem' }}
                  >
                    {effectiveSteamClientInstallPath}
                  </dd>
                </div>
              ) : null}
              <div className="crosshook-dashboard-kv-row">
                <dt className="crosshook-dashboard-kv-row__label">umu preference</dt>
                <dd className="crosshook-dashboard-kv-row__value">
                  <span className="crosshook-editor-field-readonly">
                    {profile.runtime?.umu_preference ?? settings.umu_preference}
                  </span>
                </dd>
              </div>
            </dl>
          }
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
              onApplyProtonDbEnvVars={protonDb.applyEnvVars}
              applyingProtonDbGroupId={protonDb.applyingGroupId}
              protonDbStatusMessage={protonDb.statusMessage}
              pendingProtonDbOverwrite={protonDb.pendingOverwrite}
              onConfirmProtonDbOverwrite={(overwriteKeys) => {
                if (protonDb.pendingOverwrite) {
                  protonDb.applyGroup(protonDb.pendingOverwrite.group, overwriteKeys);
                }
              }}
              onCancelProtonDbOverwrite={protonDb.clearOverwrite}
              onUpdateProtonDbResolution={(key, resolution) =>
                protonDb.updateOverwriteResolution(
                  protonDb.pendingOverwrite == null
                    ? null
                    : {
                        ...protonDb.pendingOverwrite,
                        resolutions: { ...protonDb.pendingOverwrite.resolutions, [key]: resolution },
                      }
                )
              }
              suggestionSet={suggestions.suggestionSet}
              onAcceptSuggestion={protonDb.acceptSuggestion}
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
