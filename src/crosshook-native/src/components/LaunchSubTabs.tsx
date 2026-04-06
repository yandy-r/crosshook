import { type CSSProperties, useEffect, useRef, useState } from 'react';
import * as Tabs from '@radix-ui/react-tabs';

import { CustomEnvironmentVariablesSection } from './CustomEnvironmentVariablesSection';
import GamescopeConfigPanel from './GamescopeConfigPanel';
import MangoHudConfigPanel from './MangoHudConfigPanel';
import LaunchOptimizationsPanel from './LaunchOptimizationsPanel';
import { OfflineReadinessPanel } from './OfflineReadinessPanel';
import { OfflineStatusBadge } from './OfflineStatusBadge';
import ProtonDbOverwriteConfirmation from './ProtonDbOverwriteConfirmation';
import ProtonDbLookupCard from './ProtonDbLookupCard';
import SteamLaunchOptionsPanel from './SteamLaunchOptionsPanel';
import { GameMetadataBar } from './profile-sections/GameMetadataBar';
import { useLaunchStateContext } from '../context/LaunchStateContext';
import { useGameCoverArt } from '../hooks/useGameCoverArt';
import { useImageDominantColor } from '../hooks/useImageDominantColor';
import { LAUNCH_OPTIMIZATION_APPLICABLE_METHODS } from '../types/launch-optimizations';
import type { BundledOptimizationPreset, GameProfile, LaunchAutoSaveStatus, LaunchMethod } from '../types';
import type { AcceptSuggestionRequest, ProtonDbRecommendationGroup, ProtonDbSuggestionSet } from '../types/protondb';
import type { GamescopeConfig, MangoHudConfig } from '../types/profile';
import type { LaunchOptimizationsPanelStatus } from './LaunchOptimizationsPanel';
import type { LaunchOptimizationId } from '../types/launch-optimizations';
import type { OptimizationCatalogPayload } from '../utils/optimization-catalog';
import type { PendingProtonDbOverwrite } from '../utils/protondb';

export type LaunchSubTabId =
  | 'offline'
  | 'environment'
  | 'gamescope'
  | 'mangohud'
  | 'optimizations'
  | 'steam-options';

const TAB_LABELS: Record<LaunchSubTabId, string> = {
  offline: 'Offline',
  environment: 'Environment',
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
  /** Custom cover art path from the profile, overrides Steam art when set. */
  customCoverArtPath?: string;

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

  // Environment tab — custom env vars + optional ProtonDB suggestions
  profileName: string;
  onUpdateProfile: (updater: (current: GameProfile) => GameProfile) => void;
  onEnvironmentBlurAutoSave?: (
    trigger: 'key' | 'value',
    row: Readonly<{ key: string; value: string }>,
    nextEnvVars: Readonly<Record<string, string>>
  ) => void;
  showProtonDbLookup: boolean;
  trainerVersion?: string | null;
  onApplyProtonDbEnvVars: (group: ProtonDbRecommendationGroup) => void;
  applyingProtonDbGroupId: string | null;
  protonDbStatusMessage: string | null;
  pendingProtonDbOverwrite: PendingProtonDbOverwrite | null;
  onConfirmProtonDbOverwrite: (overwriteKeys: readonly string[]) => void;
  onCancelProtonDbOverwrite: () => void;
  onUpdateProtonDbResolution: (key: string, resolution: 'keep_current' | 'use_suggestion') => void;
  suggestionSet?: ProtonDbSuggestionSet | null;
  onAcceptSuggestion?: (request: AcceptSuggestionRequest) => Promise<void>;
  onDismissSuggestion?: (suggestionKey: string) => void;

  // Auto-save status indicators
  gamescopeAutoSaveStatus?: LaunchAutoSaveStatus;
  mangoHudAutoSaveStatus?: LaunchAutoSaveStatus;
}

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
  const isNative = launchMethod === 'native';

  const launchMethodSupportsOptimizations =
    !isNative &&
    LAUNCH_OPTIMIZATION_APPLICABLE_METHODS.includes(
      launchMethod as (typeof LAUNCH_OPTIMIZATION_APPLICABLE_METHODS)[number]
    );

  const showsGamescopeTab = launchMethod === 'proton_run' || launchMethod === 'steam_applaunch';
  const showsMangoHudTab = launchMethodSupportsOptimizations;
  const showsOptimizationsTab = launchMethodSupportsOptimizations;
  const showsSteamOptionsTab = launchMethod === 'steam_applaunch';

  const tabs: LaunchSubTabId[] = isNative
    ? ['environment', 'offline']
    : [
        ...(showsOptimizationsTab ? ['optimizations' as const] : []),
        'environment',
        ...(showsMangoHudTab ? ['mangohud' as const] : []),
        ...(showsGamescopeTab ? ['gamescope' as const] : []),
        ...(showsSteamOptionsTab ? ['steam-options' as const] : []),
        'offline',
      ];

  const [activeTab, setActiveTab] = useState<LaunchSubTabId>(tabs[0] ?? 'environment');
  const autoSwitchedRef = useRef(false);

  // Unified auto-save status chip — show the highest-priority non-idle status across all tabs.
  const TONE_PRIORITY: Record<string, number> = { idle: 0, success: 1, warning: 2, saving: 3, error: 4 };
  const allStatuses: LaunchAutoSaveStatus[] = [
    launchOptimizationsStatus ?? { tone: 'idle', label: '' },
    gamescopeAutoSaveStatus ?? { tone: 'idle', label: '' },
    mangoHudAutoSaveStatus ?? { tone: 'idle', label: '' },
  ];
  const combinedAutoSaveStatus = allStatuses.reduce<LaunchAutoSaveStatus>(
    (best, s) => ((TONE_PRIORITY[s.tone] ?? 0) > (TONE_PRIORITY[best.tone] ?? 0) ? s : best),
    { tone: 'idle', label: '' }
  );
  // Fade chip out 3s after success
  const [chipVisible, setChipVisible] = useState(false);
  const chipTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => {
    if (chipTimerRef.current !== null) {
      clearTimeout(chipTimerRef.current);
      chipTimerRef.current = null;
    }
    if (combinedAutoSaveStatus.tone !== 'idle') {
      setChipVisible(true);
      if (combinedAutoSaveStatus.tone === 'success') {
        chipTimerRef.current = setTimeout(() => setChipVisible(false), 3000);
      }
    } else {
      setChipVisible(false);
    }
    return () => {
      if (chipTimerRef.current !== null) clearTimeout(chipTimerRef.current);
    };
  }, [combinedAutoSaveStatus.tone, combinedAutoSaveStatus.label]);

  useEffect(() => {
    if (tabs.length > 0 && !tabs.includes(activeTab)) {
      setActiveTab(tabs[0]);
      autoSwitchedRef.current = false;
    }
  }, [tabs.join(','), activeTab]);

  const {
    offlineReadiness,
    offlineReadinessError,
    offlineReadinessLoading,
    offlineWarning,
    launchPathWarnings,
    trainerHashUpdateBusy,
    updateStoredTrainerHash,
    dismissTrainerHashCommunityWarning,
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

  const { coverArtUrl, loading: coverArtLoading } = useGameCoverArt(steamAppId, customCoverArtPath);
  const dominantColor = useImageDominantColor(coverArtUrl);

  const gameColorStyle: CSSProperties | undefined = dominantColor
    ? {
        '--crosshook-game-color-r': String(dominantColor[0]),
        '--crosshook-game-color-g': String(dominantColor[1]),
        '--crosshook-game-color-b': String(dominantColor[2]),
      } as CSSProperties
    : undefined;

  const showCoverArt = Boolean(coverArtUrl) || coverArtLoading;

  return (
    <div className="crosshook-panel crosshook-launch-subtabs crosshook-subtabs-shell">
      <Tabs.Root
        className="crosshook-subtabs-root"
        value={activeTab}
        onValueChange={(val) => {
          // When the user manually changes the tab after an auto-switch, don't
          // re-switch back automatically on the next render cycle.
          autoSwitchedRef.current = true;
          setActiveTab(val as LaunchSubTabId);
        }}
        style={gameColorStyle}
      >
        <div
          className={[
            'crosshook-subtabs-backdrop',
            !showCoverArt ? 'crosshook-subtabs-backdrop--empty' : '',
          ]
            .filter(Boolean)
            .join(' ')}
          aria-hidden="true"
        >
          {coverArtUrl ? (
            <img
              src={coverArtUrl}
              className="crosshook-subtabs-backdrop__art"
              alt=""
              aria-hidden="true"
            />
          ) : null}
          {coverArtLoading && !coverArtUrl ? (
            <div className="crosshook-subtabs-backdrop__skeleton crosshook-skeleton" />
          ) : null}
          <div className="crosshook-subtabs-backdrop__veil" />
        </div>

        <div className="crosshook-subtabs-foreground">
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
            {chipVisible && combinedAutoSaveStatus.tone !== 'idle' ? (
              <span
                className={`crosshook-launch-autosave-chip crosshook-launch-autosave-chip--${combinedAutoSaveStatus.tone}`}
                aria-live="polite"
                aria-atomic="true"
              >
                {combinedAutoSaveStatus.label}
              </span>
            ) : null}
          </Tabs.List>

          <div className="crosshook-subtabs-metadata">
            <GameMetadataBar steamAppId={steamAppId} />
          </div>

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
                      {issue.code === 'trainer_hash_mismatch' ? (
                        <div
                          className="crosshook-launch-panel__feedback-actions"
                          style={{ marginTop: 10, display: 'flex', gap: 8, flexWrap: 'wrap' }}
                        >
                          <button
                            type="button"
                            className="crosshook-button crosshook-button--secondary"
                            disabled={trainerHashUpdateBusy}
                            onClick={() => void updateStoredTrainerHash()}
                          >
                            {trainerHashUpdateBusy ? 'Updating…' : 'Update stored hash'}
                          </button>
                        </div>
                      ) : null}
                      {issue.code === 'trainer_hash_community_mismatch' ? (
                        <div
                          className="crosshook-launch-panel__feedback-actions"
                          style={{ marginTop: 10, display: 'flex', gap: 8, flexWrap: 'wrap' }}
                        >
                          <button
                            type="button"
                            className="crosshook-button crosshook-button--secondary"
                            onClick={dismissTrainerHashCommunityWarning}
                          >
                            Dismiss
                          </button>
                        </div>
                      ) : null}
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
                  gamescopeConfig={gamescopeConfig}
                />
              </div>
            </Tabs.Content>
          ) : null}

          {/* Environment tab — custom env vars + ProtonDB lookup */}
          <Tabs.Content
            value="environment"
            forceMount
            className="crosshook-subtab-content"
            style={{ display: activeTab === 'environment' ? undefined : 'none' }}
          >
            <div className="crosshook-subtab-content__inner">
              <CustomEnvironmentVariablesSection
                profileName={profileName}
                customEnvVars={customEnvVars ?? {}}
                onUpdateProfile={onUpdateProfile}
                idPrefix="launch-subtabs"
                onAutoSaveBlur={onEnvironmentBlurAutoSave}
              />

              {showProtonDbLookup && steamAppId ? (
                <div className="crosshook-protondb-panel">
                  <ProtonDbLookupCard
                    appId={steamAppId}
                    trainerVersion={trainerVersion ?? null}
                    versionContext={null}
                    onApplyEnvVars={onApplyProtonDbEnvVars}
                    applyingGroupId={applyingProtonDbGroupId}
                    suggestionSet={suggestionSet}
                    onAcceptSuggestion={onAcceptSuggestion}
                    onDismissSuggestion={onDismissSuggestion}
                  />

                  {protonDbStatusMessage ? (
                    <p className="crosshook-help-text" role="status">
                      {protonDbStatusMessage}
                    </p>
                  ) : null}

                  {pendingProtonDbOverwrite ? (
                    <ProtonDbOverwriteConfirmation
                      pendingProtonDbOverwrite={pendingProtonDbOverwrite}
                      onUpdateProtonDbResolution={onUpdateProtonDbResolution}
                      onCancelProtonDbOverwrite={onCancelProtonDbOverwrite}
                      onConfirmProtonDbOverwrite={onConfirmProtonDbOverwrite}
                    />
                  ) : null}
                </div>
              ) : null}
            </div>
          </Tabs.Content>
        </div>
      </Tabs.Root>
    </div>
  );
}

export default LaunchSubTabs;
