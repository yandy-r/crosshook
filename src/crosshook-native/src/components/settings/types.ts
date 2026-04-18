import type { AppSettingsData } from '../../types';

/** Paths grouped by file category for the recent-files column. */
export interface RecentFilesState {
  gamePaths: string[];
  trainerPaths: string[];
  dllPaths: string[];
}

/** Props accepted by the top-level SettingsPanel composition. */
export interface SettingsPanelProps {
  settings: AppSettingsData;
  onPersistSettings: (patch: Partial<AppSettingsData>) => Promise<void>;
  recentFiles: RecentFilesState;
  targetHomePath: string;
  steamClientInstallPath: string;
  onAutoLoadLastProfileChange: (enabled: boolean) => void;
  onRefreshRecentFiles?: () => void;
  onClearRecentFiles?: () => void;
  onSteamGridDbApiKeyChange?: (key: string) => Promise<void>;
  onBrowseProfilesDirectory?: () => void;
}
