import * as Tabs from '@radix-ui/react-tabs';
import { type CSSProperties, useEffect, useRef, useState } from 'react';
import { useLaunchStateContext } from '../context/LaunchStateContext';
import { useGameCoverArt } from '../hooks/useGameCoverArt';
import { useImageDominantColor } from '../hooks/useImageDominantColor';
import { EnvironmentTabContent } from './launch-subtabs/EnvironmentTabContent';
import { GamescopeTabContent } from './launch-subtabs/GamescopeTabContent';
import { MangoHudTabContent } from './launch-subtabs/MangoHudTabContent';
import { OfflineTabContent } from './launch-subtabs/OfflineTabContent';
import { OptimizationsTabContent } from './launch-subtabs/OptimizationsTabContent';
import { SteamOptionsTabContent } from './launch-subtabs/SteamOptionsTabContent';
import { TAB_LABELS } from './launch-subtabs/types';
import { useAutoSaveChip } from './launch-subtabs/useAutoSaveChip';
import { useTabVisibility } from './launch-subtabs/useTabVisibility';
import { GameMetadataBar } from './profile-sections/GameMetadataBar';

export type { LaunchSubTabId, LaunchSubTabsProps } from './launch-subtabs/types';

import type { LaunchSubTabsProps } from './launch-subtabs/types';

export function LaunchSubTabs({
  launchMethod,
  steamAppId,
  customCoverArtPath,
  gamescopeConfig,
  onGamescopeChange,
  isInsideGamescopeSession,
  mangoHudConfig,
  onMangoHudChange,
  showMangoHudOverlayEnabled,
  enabledOptionIds,
  onToggleOption,
  launchOptimizationsStatus,
  optimizationPresetNames,
  activeOptimizationPreset,
  onSelectOptimizationPreset,
  bundledOptimizationPresets,
  onApplyBundledPreset,
  optimizationPresetActionBusy,
  onSaveManualPreset,
  catalog,
  customEnvVars,
  profileName,
  onUpdateProfile,
  onEnvironmentBlurAutoSave,
  showProtonDbLookup,
  trainerVersion,
  onApplyProtonDbEnvVars,
  applyingProtonDbGroupId,
  protonDbStatusMessage,
  pendingProtonDbOverwrite,
  onConfirmProtonDbOverwrite,
  onCancelProtonDbOverwrite,
  onUpdateProtonDbResolution,
  suggestionSet,
  onAcceptSuggestion,
  onDismissSuggestion,
  gamescopeAutoSaveStatus,
  mangoHudAutoSaveStatus,
}: LaunchSubTabsProps) {
  const { tabs, showsGamescopeTab, showsMangoHudTab, showsOptimizationsTab, showsSteamOptionsTab } =
    useTabVisibility(launchMethod);

  const [activeTab, setActiveTab] = useState(tabs[0] ?? 'environment');
  const autoSwitchedRef = useRef(false);

  // Single instance — AUTOSAVE_CHIP_MERGE_PATTERN; chip repositioned into active
  // panel header actions slot, never cloned per tab.
  const { combinedAutoSaveStatus, chipVisible } = useAutoSaveChip({
    launchOptimizationsStatus,
    gamescopeAutoSaveStatus,
    mangoHudAutoSaveStatus,
  });

  useEffect(() => {
    if (tabs.length > 0 && !tabs.includes(activeTab)) {
      setActiveTab(tabs[0]);
      autoSwitchedRef.current = false;
    }
  }, [activeTab, tabs.length, tabs[0]]);

  const { offlineWarning, launchPathWarnings, offlineReadinessError } = useLaunchStateContext();
  const hasOfflineConcern = Boolean(offlineReadinessError) || offlineWarning || launchPathWarnings.length > 0;

  // OFFLINE_AUTO_SWITCH_PATTERN — must stay in parent (needs setActiveTab)
  useEffect(() => {
    if (hasOfflineConcern && !autoSwitchedRef.current) {
      autoSwitchedRef.current = true;
      setActiveTab('offline');
    }
  }, [hasOfflineConcern]);

  // Reset the auto-switch guard whenever the concern clears.
  useEffect(() => {
    if (!hasOfflineConcern) {
      autoSwitchedRef.current = false;
    }
  }, [hasOfflineConcern]);

  const { coverArtUrl, loading: coverArtLoading } = useGameCoverArt(steamAppId, customCoverArtPath);
  const dominantColor = useImageDominantColor(coverArtUrl);

  const gameColorStyle: CSSProperties | undefined = dominantColor
    ? ({
        '--crosshook-game-color-r': String(dominantColor[0]),
        '--crosshook-game-color-g': String(dominantColor[1]),
        '--crosshook-game-color-b': String(dominantColor[2]),
      } as CSSProperties)
    : undefined;

  const showCoverArt = Boolean(coverArtUrl) || coverArtLoading;

  // Render the autosave chip only when visible and non-idle.
  // Passed as `chipSlot` to the active tab's DashboardPanelSection actions.
  const autoSaveChipSlot =
    chipVisible && combinedAutoSaveStatus.tone !== 'idle' ? (
      <span
        className={`crosshook-launch-autosave-chip crosshook-launch-autosave-chip--${combinedAutoSaveStatus.tone}`}
        aria-live="polite"
        aria-atomic="true"
      >
        {combinedAutoSaveStatus.label}
      </span>
    ) : null;

  return (
    <div className="crosshook-panel crosshook-launch-subtabs crosshook-subtabs-shell">
      <Tabs.Root
        className="crosshook-subtabs-root"
        value={activeTab}
        onValueChange={(val) => {
          // When the user manually changes the tab after an auto-switch, don't
          // re-switch back automatically on the next render cycle.
          autoSwitchedRef.current = true;
          setActiveTab(val as typeof activeTab);
        }}
        style={gameColorStyle}
      >
        <div
          className={['crosshook-subtabs-backdrop', !showCoverArt ? 'crosshook-subtabs-backdrop--empty' : '']
            .filter(Boolean)
            .join(' ')}
          aria-hidden="true"
        >
          {coverArtUrl ? (
            <img src={coverArtUrl} className="crosshook-subtabs-backdrop__art" alt="" aria-hidden="true" />
          ) : null}
          {coverArtLoading && !coverArtUrl ? (
            <div className="crosshook-subtabs-backdrop__skeleton crosshook-skeleton" />
          ) : null}
          <div className="crosshook-subtabs-backdrop__veil" />
        </div>

        <div className="crosshook-subtabs-foreground">
          <h2 className="crosshook-visually-hidden">Launch configuration</h2>
          <Tabs.List
            className={`crosshook-subtab-row${dominantColor ? ' crosshook-subtab-row--themed' : ''}`}
            aria-label="Launch configuration sections"
          >
            {tabs.map((tab) => (
              <Tabs.Trigger
                key={tab}
                value={tab}
                className={`crosshook-subtab${activeTab === tab ? ' crosshook-subtab--active' : ''}`}
              >
                {TAB_LABELS[tab]}
              </Tabs.Trigger>
            ))}
          </Tabs.List>

          <div className="crosshook-subtabs-metadata">
            <GameMetadataBar steamAppId={steamAppId} />
          </div>

          <OfflineTabContent activeTab={activeTab} chipSlot={activeTab === 'offline' ? autoSaveChipSlot : null} />

          {showsGamescopeTab ? (
            <GamescopeTabContent
              activeTab={activeTab}
              gamescopeConfig={gamescopeConfig}
              onGamescopeChange={onGamescopeChange}
              isInsideGamescopeSession={isInsideGamescopeSession}
              chipSlot={activeTab === 'gamescope' ? autoSaveChipSlot : null}
            />
          ) : null}

          {showsMangoHudTab ? (
            <MangoHudTabContent
              activeTab={activeTab}
              mangoHudConfig={mangoHudConfig}
              onMangoHudChange={onMangoHudChange}
              showMangoHudOverlayEnabled={showMangoHudOverlayEnabled}
              launchMethod={launchMethod}
              chipSlot={activeTab === 'mangohud' ? autoSaveChipSlot : null}
            />
          ) : null}

          {showsOptimizationsTab ? (
            <OptimizationsTabContent
              activeTab={activeTab}
              launchMethod={launchMethod}
              enabledOptionIds={enabledOptionIds}
              onToggleOption={onToggleOption}
              optimizationPresetNames={optimizationPresetNames}
              activeOptimizationPreset={activeOptimizationPreset}
              onSelectOptimizationPreset={onSelectOptimizationPreset}
              bundledOptimizationPresets={bundledOptimizationPresets}
              onApplyBundledPreset={onApplyBundledPreset}
              optimizationPresetActionBusy={optimizationPresetActionBusy}
              onSaveManualPreset={onSaveManualPreset}
              catalog={catalog}
              chipSlot={activeTab === 'optimizations' ? autoSaveChipSlot : null}
            />
          ) : null}

          {showsSteamOptionsTab ? (
            <SteamOptionsTabContent
              activeTab={activeTab}
              enabledOptionIds={enabledOptionIds}
              customEnvVars={customEnvVars}
              gamescopeConfig={gamescopeConfig}
              chipSlot={activeTab === 'steam-options' ? autoSaveChipSlot : null}
            />
          ) : null}

          <EnvironmentTabContent
            activeTab={activeTab}
            profileName={profileName}
            customEnvVars={customEnvVars}
            onUpdateProfile={onUpdateProfile}
            onEnvironmentBlurAutoSave={onEnvironmentBlurAutoSave}
            showProtonDbLookup={showProtonDbLookup}
            steamAppId={steamAppId}
            trainerVersion={trainerVersion}
            onApplyProtonDbEnvVars={onApplyProtonDbEnvVars}
            applyingProtonDbGroupId={applyingProtonDbGroupId}
            protonDbStatusMessage={protonDbStatusMessage}
            pendingProtonDbOverwrite={pendingProtonDbOverwrite}
            onConfirmProtonDbOverwrite={onConfirmProtonDbOverwrite}
            onCancelProtonDbOverwrite={onCancelProtonDbOverwrite}
            onUpdateProtonDbResolution={onUpdateProtonDbResolution}
            suggestionSet={suggestionSet}
            onAcceptSuggestion={onAcceptSuggestion}
            onDismissSuggestion={onDismissSuggestion}
            chipSlot={activeTab === 'environment' ? autoSaveChipSlot : null}
          />
        </div>
      </Tabs.Root>
    </div>
  );
}

export default LaunchSubTabs;
