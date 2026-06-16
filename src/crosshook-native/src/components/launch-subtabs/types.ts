import type { BundledOptimizationPreset, LaunchAutoSaveStatus, LaunchMethod } from '../../types';
import type { CommandArgumentCatalogPayload, LaunchCommandArguments } from '../../types/launch-command-arguments';
import type { LaunchOptimizationId } from '../../types/launch-optimizations';
import type { GameProfile, GamescopeConfig, MangoHudConfig } from '../../types/profile';
import type { AcceptSuggestionRequest, ProtonDbRecommendationGroup, ProtonDbSuggestionSet } from '../../types/protondb';
import type { OptimizationCatalogPayload } from '../../utils/optimization-catalog';
import type { PendingProtonDbOverwrite } from '../../utils/protondb';
import type { LaunchOptimizationsPanelStatus } from '../LaunchOptimizationsPanel';

export type LaunchSubTabId =
  | 'offline'
  | 'environment'
  | 'gamescope'
  | 'mangohud'
  | 'optimizations'
  | 'command-arguments'
  | 'steam-options';

export const TAB_LABELS: Record<LaunchSubTabId, string> = {
  offline: 'Offline',
  environment: 'Environment',
  gamescope: 'Gamescope',
  mangohud: 'MangoHud',
  optimizations: 'Optimizations',
  'command-arguments': 'Command Arguments',
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

  // Command Arguments panel (Task 5.2 UI; state wired in Task 5.1)
  commandArguments?: LaunchCommandArguments;
  onToggleCommandArgument?: (argumentId: string, nextEnabled: boolean) => void;
  onUpdateCommandArgumentsCustomArgs?: (customArgs: readonly string[]) => void;
  commandArgumentCatalog?: CommandArgumentCatalogPayload | null;
  commandArgumentsAutoSaveStatus?: LaunchAutoSaveStatus;

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
