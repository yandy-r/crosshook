import { useCallback, useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

import LaunchPanel from '../LaunchPanel';
import { LaunchSubTabs } from '../LaunchSubTabs';
import { ThemedSelect } from '../ui/ThemedSelect';
import { useProfileContext } from '../../context/ProfileContext';
import { PageBanner, LaunchArt } from '../layout/PageBanner';
import { DEFAULT_GAMESCOPE_CONFIG, DEFAULT_MANGOHUD_CONFIG } from '../../types/profile';
import { buildProfileLaunchRequest } from '../../utils/launch';

export function LaunchPage() {
  const profileState = useProfileContext();
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

  return (
    <div className="crosshook-page-scroll-shell crosshook-page-scroll-shell--launch">
      <PageBanner
        eyebrow="Launch"
        title="Launch controls"
        copy="Start the selected profile through its current runtime method without the install-flow override from the old shell."
        illustration={<LaunchArt />}
      />

      <div className="crosshook-launch-page__grid">
        <LaunchPanel
          profileId={profileId}
          method={profileState.launchMethod}
          request={launchRequest}
          beforeActions={
            <section style={{ marginTop: 16 }}>
              <span
                id="active-profile-label"
                className="crosshook-heading-eyebrow"
                style={{ marginBottom: 8, display: 'block' }}
              >
                Active Profile
              </span>
              <ThemedSelect
                value={profileState.selectedProfile}
                onValueChange={(name) => void profileState.selectProfile(name)}
                placeholder="Select a profile"
                pinnedValues={pinnedSet}
                onTogglePin={handleTogglePin}
                ariaLabelledby="active-profile-label"
                options={profileState.profiles.map((name) => ({ value: name, label: name }))}
              />
            </section>
          }
          tabsSlot={
            <LaunchSubTabs
              launchMethod={profileState.launchMethod}
              steamAppId={profile.steam.app_id}
              customCoverArtPath={profile.game.custom_cover_art_path}
              gamescopeConfig={profile.launch.gamescope ?? DEFAULT_GAMESCOPE_CONFIG}
              onGamescopeChange={(gamescope) => {
                profileState.updateProfile((current) => ({
                  ...current,
                  launch: { ...current.launch, gamescope },
                }));
              }}
              isInsideGamescopeSession={isInsideGamescopeSession}
              mangoHudConfig={profile.launch.mangohud ?? DEFAULT_MANGOHUD_CONFIG}
              onMangoHudChange={(mangohud) => {
                profileState.updateProfile((current) => ({
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
            />
          }
        />
      </div>
    </div>
  );
}

export default LaunchPage;
