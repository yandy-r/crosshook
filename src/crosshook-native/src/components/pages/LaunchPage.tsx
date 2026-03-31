import { useCallback, useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

import GamescopeConfigPanel from '../GamescopeConfigPanel';
import LaunchOptimizationsPanel from '../LaunchOptimizationsPanel';
import LaunchPanel from '../LaunchPanel';
import { PinnedProfilesStrip } from '../PinnedProfilesStrip';
import SteamLaunchOptionsPanel from '../SteamLaunchOptionsPanel';
import { CollapsibleSection } from '../ui/CollapsibleSection';
import { ThemedSelect } from '../ui/ThemedSelect';
import { useProfileContext } from '../../context/ProfileContext';
import { PageBanner, LaunchArt } from '../layout/PageBanner';
import { DEFAULT_GAMESCOPE_CONFIG } from '../../types/profile';
import { buildProfileLaunchRequest } from '../../utils/launch';

export function LaunchPage() {
  const profileState = useProfileContext();
  const profile = profileState.profile;
  const selectedName = profileState.selectedProfile || '';
  const launchRequest = buildProfileLaunchRequest(
    profile,
    profileState.launchMethod,
    profileState.steamClientInstallPath,
    selectedName,
  );
  const profileId = profileState.profileName.trim() || selectedName || 'new-profile';
  const [isInsideGamescopeSession, setIsInsideGamescopeSession] = useState(false);
  useEffect(() => {
    invoke<boolean>('check_gamescope_session')
      .then(setIsInsideGamescopeSession)
      .catch(() => {});
  }, []);

  const showsGamescopePanel = profileState.launchMethod === 'proton_run';
  const showsOptimizationPanels =
    profileState.launchMethod === 'proton_run' || profileState.launchMethod === 'steam_applaunch';
  const showsSteamLaunchOptions = profileState.launchMethod === 'steam_applaunch';
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

  return (
    <>
      <PageBanner
        eyebrow="Launch"
        title="Launch controls"
        copy="Start the selected profile through its current runtime method without the install-flow override from the old shell."
        illustration={<LaunchArt />}
      />

      <div style={{ display: 'grid', gap: 24 }}>
        <CollapsibleSection title="Launch Controls" className="crosshook-panel">
          <LaunchPanel profileId={profileId} method={profileState.launchMethod} request={launchRequest} />
        </CollapsibleSection>

        <section className="crosshook-launch-profile-selector">
          <span className="crosshook-heading-eyebrow">Active Profile</span>
          <ThemedSelect
            value={profileState.selectedProfile}
            onValueChange={(name) => void profileState.selectProfile(name)}
            placeholder="Select a profile"
            pinnedValues={pinnedSet}
            onTogglePin={handleTogglePin}
            options={profileState.profiles.map((name) => ({ value: name, label: name }))}
          />
        </section>

        {profileState.favoriteProfiles.length > 0 ? (
          <section className="crosshook-panel">
            <PinnedProfilesStrip
              favoriteProfiles={profileState.favoriteProfiles}
              selectedProfile={profileState.selectedProfile}
              onSelectProfile={profileState.selectProfile}
              onToggleFavorite={profileState.toggleFavorite}
            />
          </section>
        ) : null}

        {showsGamescopePanel ? (
          <CollapsibleSection title="Gamescope" className="crosshook-panel" defaultOpen={false}>
            <GamescopeConfigPanel
              config={profile.launch.gamescope ?? DEFAULT_GAMESCOPE_CONFIG}
              onChange={(gamescope) => {
                profileState.updateProfile((current) => ({
                  ...current,
                  launch: { ...current.launch, gamescope },
                }));
              }}
              isInsideGamescopeSession={isInsideGamescopeSession}
            />
          </CollapsibleSection>
        ) : null}

        {showsOptimizationPanels ? (
          <CollapsibleSection title="Launch Optimizations" className="crosshook-panel">
            <LaunchOptimizationsPanel
              method={profileState.launchMethod}
              enabledOptionIds={profile.launch.optimizations.enabled_option_ids}
              onToggleOption={profileState.toggleLaunchOptimization}
              status={profileState.launchOptimizationsStatus}
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
            />
          </CollapsibleSection>
        ) : null}

        {showsSteamLaunchOptions ? (
          <CollapsibleSection title="Steam Launch Options" className="crosshook-panel">
            <SteamLaunchOptionsPanel
              enabledOptionIds={profile.launch.optimizations.enabled_option_ids}
              customEnvVars={profile.launch.custom_env_vars}
            />
          </CollapsibleSection>
        ) : null}
      </div>
    </>
  );
}

export default LaunchPage;
