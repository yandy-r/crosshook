import { useCallback, useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

import GamescopeConfigPanel from '../GamescopeConfigPanel';
import MangoHudConfigPanel from '../MangoHudConfigPanel';
import LaunchOptimizationsPanel from '../LaunchOptimizationsPanel';
import LaunchPanel from '../LaunchPanel';
import { OfflineReadinessPanel } from '../OfflineReadinessPanel';
import { OfflineStatusBadge } from '../OfflineStatusBadge';
import { PinnedProfilesStrip } from '../PinnedProfilesStrip';
import SteamLaunchOptionsPanel from '../SteamLaunchOptionsPanel';
import { CollapsibleSection } from '../ui/CollapsibleSection';
import { ThemedSelect } from '../ui/ThemedSelect';
import { useLaunchStateContext } from '../../context/LaunchStateContext';
import { useProfileContext } from '../../context/ProfileContext';
import { PageBanner, LaunchArt } from '../layout/PageBanner';
import { DEFAULT_GAMESCOPE_CONFIG, DEFAULT_MANGOHUD_CONFIG } from '../../types/profile';
import { LAUNCH_OPTIMIZATION_APPLICABLE_METHODS } from '../../types/launch-optimizations';
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

  const showsGamescopePanel = profileState.launchMethod === 'proton_run';
  const launchMethodSupportsOptimizations = LAUNCH_OPTIMIZATION_APPLICABLE_METHODS.includes(
    profileState.launchMethod as (typeof LAUNCH_OPTIMIZATION_APPLICABLE_METHODS)[number]
  );
  const showsMangoHudPanel = launchMethodSupportsOptimizations;
  const showsOptimizationPanels = launchMethodSupportsOptimizations;
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
          <LaunchPanel
            profileId={profileId}
            method={profileState.launchMethod}
            request={launchRequest}
            infoSlot={
              profileState.favoriteProfiles.length > 0 ? (
                <PinnedProfilesStrip
                  favoriteProfiles={profileState.favoriteProfiles}
                  selectedProfile={profileState.selectedProfile}
                  onSelectProfile={profileState.selectProfile}
                  onToggleFavorite={profileState.toggleFavorite}
                />
              ) : null
            }
            beforeActions={
              <section style={{ marginTop: 16 }}>
                <span className="crosshook-heading-eyebrow" style={{ marginBottom: 8, display: 'block' }}>Active Profile</span>
                <ThemedSelect
                  value={profileState.selectedProfile}
                  onValueChange={(name) => void profileState.selectProfile(name)}
                  placeholder="Select a profile"
                  pinnedValues={pinnedSet}
                  onTogglePin={handleTogglePin}
                  options={profileState.profiles.map((name) => ({ value: name, label: name }))}
                />
              </section>
            }
          />
        </CollapsibleSection>

        <OfflineReadinessSection method={profileState.launchMethod} />

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

        {showsMangoHudPanel ? (
          <CollapsibleSection title="MangoHud Overlay Config" className="crosshook-panel" defaultOpen={false}>
            <MangoHudConfigPanel
              config={profile.launch.mangohud ?? DEFAULT_MANGOHUD_CONFIG}
              onChange={(mangohud) => {
                profileState.updateProfile((current) => ({
                  ...current,
                  launch: { ...current.launch, mangohud },
                }));
              }}
              showMangoHudOverlayEnabled={profile.launch.optimizations.enabled_option_ids.includes(
                'show_mangohud_overlay'
              )}
              launchMethod={profileState.launchMethod}
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
              catalog={profileState.catalog}
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

function OfflineReadinessSection({ method }: { method: string }) {
  const {
    offlineReadiness,
    offlineReadinessError,
    offlineReadinessLoading,
    offlineWarning,
    launchPathWarnings,
  } = useLaunchStateContext();

  const hasOfflineConcern = Boolean(offlineReadinessError) || offlineWarning || launchPathWarnings.length > 0;
  const [open, setOpen] = useState(false);

  useEffect(() => {
    if (hasOfflineConcern) {
      setOpen(true);
    }
  }, [hasOfflineConcern]);

  if (method === 'native') return null;

  return (
    <CollapsibleSection
      title="Offline readiness"
      className="crosshook-panel crosshook-launch-panel__offline"
      open={open}
      onToggle={setOpen}
      meta={
        <span style={{ display: 'inline-flex', alignItems: 'center', gap: 10, flexWrap: 'wrap' }}>
          <OfflineStatusBadge report={offlineReadiness} loading={offlineReadinessLoading && !offlineReadiness} />
          {!offlineReadinessLoading && offlineReadiness ? (
            <span className="crosshook-muted" style={{ fontSize: '0.85rem' }}>
              {offlineReadiness.readiness_state.replace(/_/g, ' ')}
            </span>
          ) : null}
        </span>
      }
    >
      <OfflineReadinessPanel
        report={offlineReadiness}
        error={offlineReadinessError}
        loading={offlineReadinessLoading}
      />
      {launchPathWarnings.length > 0 ? (
        <ul className="crosshook-launch-panel__feedback-list" aria-label="Launch path warnings">
          {launchPathWarnings.map((issue, index) => (
            <li key={`launch-warn-${issue.message}-${index}`} className="crosshook-launch-panel__feedback-item">
              <div className="crosshook-launch-panel__feedback-header">
                <span className="crosshook-launch-panel__feedback-badge" data-severity={issue.severity}>
                  {issue.severity}
                </span>
                <p className="crosshook-launch-panel__feedback-title">{issue.message}</p>
              </div>
              <p className="crosshook-launch-panel__feedback-help">{issue.help}</p>
            </li>
          ))}
        </ul>
      ) : null}
    </CollapsibleSection>
  );
}

export default LaunchPage;
