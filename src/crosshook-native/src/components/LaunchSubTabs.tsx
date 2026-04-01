import { type CSSProperties, useEffect, useRef, useState } from 'react';
import * as Tabs from '@radix-ui/react-tabs';

import GamescopeConfigPanel from './GamescopeConfigPanel';
import MangoHudConfigPanel from './MangoHudConfigPanel';
import LaunchOptimizationsPanel from './LaunchOptimizationsPanel';
import { OfflineReadinessPanel } from './OfflineReadinessPanel';
import { OfflineStatusBadge } from './OfflineStatusBadge';
import SteamLaunchOptionsPanel from './SteamLaunchOptionsPanel';
import { GameMetadataBar } from './profile-sections/GameMetadataBar';
import { useLaunchStateContext } from '../context/LaunchStateContext';
import { useGameCoverArt } from '../hooks/useGameCoverArt';
import { useImageDominantColor } from '../hooks/useImageDominantColor';
import { LAUNCH_OPTIMIZATION_APPLICABLE_METHODS } from '../types/launch-optimizations';
import type { BundledOptimizationPreset, LaunchMethod } from '../types';
import type { GamescopeConfig, MangoHudConfig } from '../types/profile';
import type { LaunchOptimizationsPanelStatus } from './LaunchOptimizationsPanel';
import type { LaunchOptimizationId } from '../types/launch-optimizations';
import type { OptimizationCatalogPayload } from '../utils/optimization-catalog';

export type LaunchSubTabId = 'offline' | 'gamescope' | 'mangohud' | 'optimizations' | 'steam-options';

const TAB_LABELS: Record<LaunchSubTabId, string> = {
  offline: 'Offline',
  gamescope: 'Gamescope',
  mangohud: 'MangoHud',
  optimizations: 'Optimizations',
  'steam-options': 'Steam Options',
};

export interface LaunchSubTabsProps {
  /** Active launch method (proton_run, steam_applaunch, native, etc.). */
  launchMethod: LaunchMethod;
  /** Steam App ID from the active profile, used to load cover art and game metadata. */
  steamAppId: string | undefined;

  // Gamescope panel
  gamescopeConfig: GamescopeConfig;
  onGamescopeChange: (config: GamescopeConfig) => void;
  isInsideGamescopeSession: boolean;

  // MangoHud panel
  mangoHudConfig: MangoHudConfig;
  onMangoHudChange: (config: MangoHudConfig) => void;
  showMangoHudOverlayEnabled: boolean;

  // Launch Optimizations panel
  enabledOptionIds: readonly LaunchOptimizationId[];
  onToggleOption: (optionId: LaunchOptimizationId, nextEnabled: boolean) => void;
  launchOptimizationsStatus?: LaunchOptimizationsPanelStatus;
  optimizationPresetNames?: readonly string[];
  activeOptimizationPreset?: string;
  onSelectOptimizationPreset?: (presetName: string) => void;
  bundledOptimizationPresets?: readonly BundledOptimizationPreset[];
  onApplyBundledPreset?: (presetId: string) => void;
  optimizationPresetActionBusy?: boolean;
  onSaveManualPreset?: (presetName: string) => Promise<void>;
  catalog: OptimizationCatalogPayload | null;

  // Steam Launch Options panel
  customEnvVars?: Readonly<Record<string, string>>;
}

export function LaunchSubTabs({
  launchMethod,
  steamAppId,
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
}: LaunchSubTabsProps) {
  const isNative = launchMethod === 'native';

  const launchMethodSupportsOptimizations =
    !isNative &&
    LAUNCH_OPTIMIZATION_APPLICABLE_METHODS.includes(
      launchMethod as (typeof LAUNCH_OPTIMIZATION_APPLICABLE_METHODS)[number]
    );

  const showsGamescopeTab = launchMethod === 'proton_run';
  const showsMangoHudTab = launchMethodSupportsOptimizations;
  const showsOptimizationsTab = launchMethodSupportsOptimizations;
  const showsSteamOptionsTab = launchMethod === 'steam_applaunch';

  const tabs: LaunchSubTabId[] = isNative
    ? []
    : [
        ...(showsOptimizationsTab ? ['optimizations' as const] : []),
        ...(showsMangoHudTab ? ['mangohud' as const] : []),
        ...(showsGamescopeTab ? ['gamescope' as const] : []),
        ...(showsSteamOptionsTab ? ['steam-options' as const] : []),
        'offline',
      ];

  const [activeTab, setActiveTab] = useState<LaunchSubTabId>(tabs[0] ?? 'optimizations');
  const autoSwitchedRef = useRef(false);

  const {
    offlineReadiness,
    offlineReadinessError,
    offlineReadinessLoading,
    offlineWarning,
    launchPathWarnings,
  } = useLaunchStateContext();

  const hasOfflineConcern =
    Boolean(offlineReadinessError) || offlineWarning || launchPathWarnings.length > 0;

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

  const { coverArtUrl, loading: coverArtLoading } = useGameCoverArt(steamAppId);
  const dominantColor = useImageDominantColor(coverArtUrl);

  const gameColorStyle: CSSProperties | undefined = dominantColor
    ? {
        '--crosshook-game-color-r': String(dominantColor[0]),
        '--crosshook-game-color-g': String(dominantColor[1]),
        '--crosshook-game-color-b': String(dominantColor[2]),
      } as CSSProperties
    : undefined;

  const hasHero = Boolean(coverArtUrl) || coverArtLoading;

  // No tabs for native method — return null after all hooks have been called.
  if (isNative) return null;

  return (
    <div className="crosshook-panel" style={{ padding: 'var(--crosshook-card-padding)' }}>
      <Tabs.Root
        value={activeTab}
        onValueChange={(val) => {
          // When the user manually changes the tab after an auto-switch, don't
          // re-switch back automatically on the next render cycle.
          autoSwitchedRef.current = true;
          setActiveTab(val as LaunchSubTabId);
        }}
        style={gameColorStyle}
      >
        {/* Blended cover art hero banner */}
        {hasHero ? (
          <div className="crosshook-profile-hero">
            {coverArtUrl ? (
              <>
                <img
                  src={coverArtUrl}
                  className="crosshook-profile-hero__art"
                  alt=""
                  aria-hidden="true"
                />
                <div className="crosshook-profile-hero__gradient" />
              </>
            ) : (
              <div className="crosshook-profile-hero__skeleton crosshook-skeleton" />
            )}
            <div className="crosshook-profile-hero__content">
              <GameMetadataBar steamAppId={steamAppId} />
            </div>
          </div>
        ) : null}

        {/* Tab row — themed when game color is available */}
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

        {/* Offline tab */}
        <Tabs.Content
          value="offline"
          forceMount
          className="crosshook-subtab-content"
          style={{ display: activeTab === 'offline' ? undefined : 'none' }}
        >
          <div className="crosshook-subtab-content__inner">
            <div
              className="crosshook-launch-panel__offline"
              style={{ display: 'flex', alignItems: 'center', gap: 10, flexWrap: 'wrap', marginBottom: 12 }}
            >
              <OfflineStatusBadge
                report={offlineReadiness}
                loading={offlineReadinessLoading && !offlineReadiness}
              />
              {!offlineReadinessLoading && offlineReadiness ? (
                <span className="crosshook-muted" style={{ fontSize: '0.85rem' }}>
                  {offlineReadiness.readiness_state.replace(/_/g, ' ')}
                </span>
              ) : null}
            </div>
            <OfflineReadinessPanel
              report={offlineReadiness}
              error={offlineReadinessError}
              loading={offlineReadinessLoading}
            />
            {launchPathWarnings.length > 0 ? (
              <ul className="crosshook-launch-panel__feedback-list" aria-label="Launch path warnings">
                {launchPathWarnings.map((issue, index) => (
                  <li
                    key={`launch-warn-${issue.message}-${index}`}
                    className="crosshook-launch-panel__feedback-item"
                  >
                    <div className="crosshook-launch-panel__feedback-header">
                      <span
                        className="crosshook-launch-panel__feedback-badge"
                        data-severity={issue.severity}
                      >
                        {issue.severity}
                      </span>
                      <p className="crosshook-launch-panel__feedback-title">{issue.message}</p>
                    </div>
                    <p className="crosshook-launch-panel__feedback-help">{issue.help}</p>
                  </li>
                ))}
              </ul>
            ) : null}
          </div>
        </Tabs.Content>

        {/* Gamescope tab — proton_run only */}
        {showsGamescopeTab ? (
          <Tabs.Content
            value="gamescope"
            forceMount
            className="crosshook-subtab-content"
            style={{ display: activeTab === 'gamescope' ? undefined : 'none' }}
          >
            <div className="crosshook-subtab-content__inner">
              <GamescopeConfigPanel
                config={gamescopeConfig}
                onChange={onGamescopeChange}
                isInsideGamescopeSession={isInsideGamescopeSession}
              />
            </div>
          </Tabs.Content>
        ) : null}

        {/* MangoHud tab — proton_run or steam_applaunch */}
        {showsMangoHudTab ? (
          <Tabs.Content
            value="mangohud"
            forceMount
            className="crosshook-subtab-content"
            style={{ display: activeTab === 'mangohud' ? undefined : 'none' }}
          >
            <div className="crosshook-subtab-content__inner">
              <MangoHudConfigPanel
                config={mangoHudConfig}
                onChange={onMangoHudChange}
                showMangoHudOverlayEnabled={showMangoHudOverlayEnabled}
                launchMethod={launchMethod}
              />
            </div>
          </Tabs.Content>
        ) : null}

        {/* Optimizations tab — proton_run or steam_applaunch */}
        {showsOptimizationsTab ? (
          <Tabs.Content
            value="optimizations"
            forceMount
            className="crosshook-subtab-content"
            style={{ display: activeTab === 'optimizations' ? undefined : 'none' }}
          >
            <div className="crosshook-subtab-content__inner">
              <LaunchOptimizationsPanel
                method={launchMethod}
                enabledOptionIds={enabledOptionIds}
                onToggleOption={onToggleOption}
                status={launchOptimizationsStatus}
                optimizationPresetNames={optimizationPresetNames}
                activeOptimizationPreset={activeOptimizationPreset}
                onSelectOptimizationPreset={onSelectOptimizationPreset}
                bundledOptimizationPresets={bundledOptimizationPresets}
                onApplyBundledPreset={onApplyBundledPreset}
                optimizationPresetActionBusy={optimizationPresetActionBusy}
                onSaveManualPreset={onSaveManualPreset}
                catalog={catalog}
              />
            </div>
          </Tabs.Content>
        ) : null}

        {/* Steam Options tab — steam_applaunch only */}
        {showsSteamOptionsTab ? (
          <Tabs.Content
            value="steam-options"
            forceMount
            className="crosshook-subtab-content"
            style={{ display: activeTab === 'steam-options' ? undefined : 'none' }}
          >
            <div className="crosshook-subtab-content__inner">
              <SteamLaunchOptionsPanel
                enabledOptionIds={enabledOptionIds}
                customEnvVars={customEnvVars}
              />
            </div>
          </Tabs.Content>
        ) : null}
      </Tabs.Root>
    </div>
  );
}

export default LaunchSubTabs;
