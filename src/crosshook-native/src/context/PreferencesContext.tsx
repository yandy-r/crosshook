/**
 * Application-wide settings and recent file history.
 *
 * PreferencesContext owns app settings (auto-load, community taps) and recent file
 * paths. Profile CRUD and selection are handled by ProfileContext.
 */
import React, { createContext, type ReactNode, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { callCommand } from '@/lib/ipc';
import type { AppSettingsData, RecentFilesData } from '../types';
import type { LibraryViewMode } from '../types/library';
import { DEFAULT_APP_SETTINGS, toSettingsSaveRequest } from '../types/settings';

export interface PreferencesContextValue {
  settings: AppSettingsData;
  recentFiles: RecentFilesData;
  settingsError: string | null;
  defaultSteamClientInstallPath: string;
  refreshPreferences: () => Promise<void>;
  /** Merge partial settings, save to settings.toml, reload computed IPC fields. */
  persistSettings: (patch: Partial<AppSettingsData>) => Promise<void>;
  handleAutoLoadChange: (enabled: boolean) => Promise<void>;
  handleSteamGridDbApiKeyChange: (key: string) => Promise<void>;
  clearRecentFiles: () => Promise<void>;
  /** Library view mode from `[ui].library_view_mode`; `'grid'` while settings load or when unset. */
  libraryViewMode: LibraryViewMode;
  /** Optimistically updates `libraryViewMode` and persists via `settings_set_library_view_mode` (fire-and-forget). */
  setLibraryViewMode: (mode: LibraryViewMode) => void;
}

export interface PreferencesProviderProps {
  children: ReactNode;
  activeProfileName?: string;
}

const EMPTY_RECENT_FILES: RecentFilesData = {
  game_paths: [],
  trainer_paths: [],
  dll_paths: [],
};

const PreferencesContext = createContext<PreferencesContextValue | null>(null);

function formatError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** Snapshot returned by the initial/refresh preferences load. */
interface LoadedPreferences {
  loadedSettings: AppSettingsData;
  loadedRecentFiles: RecentFilesData;
  steamClientInstallPath: string;
}

async function loadPreferences(): Promise<LoadedPreferences> {
  const [loadedSettings, loadedRecentFiles, steamClientInstallPath] = await Promise.all([
    callCommand<AppSettingsData>('settings_load'),
    callCommand<RecentFilesData>('recent_files_load'),
    callCommand<string>('default_steam_client_install_path'),
  ]);

  return {
    loadedSettings,
    loadedRecentFiles,
    steamClientInstallPath,
  };
}

export function PreferencesProvider({ children, activeProfileName }: PreferencesProviderProps) {
  const [settings, setSettings] = useState<AppSettingsData>(DEFAULT_APP_SETTINGS);
  const [recentFiles, setRecentFiles] = useState<RecentFilesData>(EMPTY_RECENT_FILES);
  const [settingsError, setSettingsError] = useState<string | null>(null);
  const [defaultSteamClientInstallPath, setDefaultSteamClientInstallPath] = useState('');

  const pendingViewModeRef = useRef<LibraryViewMode | null>(null);
  const viewModePersistedRef = useRef(false);

  // Applies a freshly loaded settings snapshot, preserving the last requested
  // library view mode while its persist is unconfirmed. The pending guard lifts
  // once the persist has settled AND a load confirms the mode on the backend
  // (a stale load resolving with a different value keeps the optimistic mode).
  const applyLoadedSettings = useCallback((loadedSettings: AppSettingsData) => {
    const pendingViewMode = pendingViewModeRef.current;
    if (pendingViewMode === null) {
      setSettings(loadedSettings);
      return;
    }
    if (loadedSettings.ui.library_view_mode === pendingViewMode && viewModePersistedRef.current) {
      pendingViewModeRef.current = null;
      setSettings(loadedSettings);
      return;
    }
    setSettings({
      ...loadedSettings,
      ui: { ...loadedSettings.ui, library_view_mode: pendingViewMode },
    });
  }, []);

  const applyLoadedPreferences = useCallback(
    (nextPreferences: LoadedPreferences) => {
      applyLoadedSettings(nextPreferences.loadedSettings);
      setRecentFiles(nextPreferences.loadedRecentFiles);
      setDefaultSteamClientInstallPath(nextPreferences.steamClientInstallPath);
      setSettingsError(null);
    },
    [applyLoadedSettings]
  );

  const refreshPreferences = useCallback(async () => {
    try {
      applyLoadedPreferences(await loadPreferences());
    } catch (error) {
      setSettingsError(formatError(error));
      throw error;
    }
  }, [applyLoadedPreferences]);

  useEffect(() => {
    let active = true;

    async function initializePreferences() {
      try {
        const nextPreferences = await loadPreferences();
        if (!active) {
          return;
        }

        applyLoadedPreferences(nextPreferences);
      } catch (error) {
        if (active) {
          setSettingsError(formatError(error));
        }
      }
    }

    void initializePreferences();

    return () => {
      active = false;
    };
  }, [applyLoadedPreferences]);

  const persistSettings = useCallback(
    async (patch: Partial<AppSettingsData>) => {
      const merged: AppSettingsData = { ...settings, ...patch };
      await callCommand('settings_save', { data: toSettingsSaveRequest(merged) });
      const loaded = await callCommand<AppSettingsData>('settings_load');
      applyLoadedSettings(loaded);
      setSettingsError(null);
    },
    [applyLoadedSettings, settings]
  );

  const handleAutoLoadChange = useCallback(
    async (enabled: boolean) => {
      try {
        await persistSettings({
          auto_load_last_profile: enabled,
          last_used_profile: activeProfileName?.trim() || settings.last_used_profile,
        });
      } catch (error) {
        setSettingsError(formatError(error));
        throw error;
      }
    },
    [activeProfileName, persistSettings, settings.last_used_profile]
  );

  const handleSteamGridDbApiKeyChange = useCallback(async (key: string) => {
    const trimmedKey = key.trim();
    const keyOrNull = trimmedKey.length > 0 ? trimmedKey : null;

    try {
      await callCommand('settings_save_steamgriddb_key', { key: keyOrNull });
      setSettings((previous) => ({
        ...previous,
        has_steamgriddb_api_key: keyOrNull !== null,
      }));
      setSettingsError(null);
    } catch (error) {
      setSettingsError(formatError(error));
      throw error;
    }
  }, []);

  const clearRecentFiles = useCallback(async () => {
    const nextRecentFiles = {
      game_paths: [],
      trainer_paths: [],
      dll_paths: [],
    } satisfies RecentFilesData;

    try {
      await callCommand('recent_files_save', { data: nextRecentFiles });
      setRecentFiles(nextRecentFiles);
      setSettingsError(null);
    } catch (error) {
      setSettingsError(formatError(error));
      throw error;
    }
  }, []);

  const libraryViewMode = settings.ui.library_view_mode ?? 'grid';

  const setLibraryViewMode = useCallback((mode: LibraryViewMode) => {
    // Track the requested mode until the persist has settled AND a load confirms
    // it, so a late settings_load resolution cannot undo the optimistic toggle.
    pendingViewModeRef.current = mode;
    viewModePersistedRef.current = false;
    setSettings((previous) => ({
      ...previous,
      ui: { ...previous.ui, library_view_mode: mode },
    }));
    void callCommand('settings_set_library_view_mode', { mode })
      .then(() => {
        viewModePersistedRef.current = true;
      })
      .catch((error: unknown) => {
        console.error('Failed to persist library view mode', error);
        // The write never landed: drop the guard so the next load applies as written.
        if (pendingViewModeRef.current === mode) {
          pendingViewModeRef.current = null;
        }
      });
  }, []);

  const value = useMemo<PreferencesContextValue>(
    () => ({
      settings,
      recentFiles,
      settingsError,
      defaultSteamClientInstallPath,
      refreshPreferences,
      persistSettings,
      handleAutoLoadChange,
      handleSteamGridDbApiKeyChange,
      clearRecentFiles,
      libraryViewMode,
      setLibraryViewMode,
    }),
    [
      clearRecentFiles,
      defaultSteamClientInstallPath,
      handleAutoLoadChange,
      handleSteamGridDbApiKeyChange,
      libraryViewMode,
      persistSettings,
      recentFiles,
      refreshPreferences,
      setLibraryViewMode,
      settings,
      settingsError,
    ]
  );

  return <PreferencesContext.Provider value={value}>{children}</PreferencesContext.Provider>;
}

export function usePreferencesContext(): PreferencesContextValue {
  const context = React.useContext(PreferencesContext);

  if (context === null) {
    throw new Error('usePreferencesContext must be used within a PreferencesProvider');
  }

  return context;
}
