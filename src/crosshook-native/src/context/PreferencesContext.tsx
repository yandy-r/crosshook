/**
 * Application-wide settings and recent file history.
 *
 * PreferencesContext owns app settings (auto-load, community taps) and recent file
 * paths. Profile CRUD and selection are handled by ProfileContext.
 */
import React, { createContext, useCallback, useEffect, useMemo, useState, type ReactNode } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { AppSettingsData, RecentFilesData } from '../types';

export interface PreferencesContextValue {
  settings: AppSettingsData;
  recentFiles: RecentFilesData;
  settingsError: string | null;
  defaultSteamClientInstallPath: string;
  refreshPreferences: () => Promise<void>;
  handleAutoLoadChange: (enabled: boolean) => Promise<void>;
  handleSteamGridDbApiKeyChange: (key: string) => Promise<void>;
  clearRecentFiles: () => Promise<void>;
}

export interface PreferencesProviderProps {
  children: ReactNode;
  activeProfileName?: string;
}

const EMPTY_SETTINGS: AppSettingsData = {
  auto_load_last_profile: false,
  last_used_profile: '',
  community_taps: [],
  onboarding_completed: false,
  offline_mode: false,
  has_steamgriddb_api_key: false,
};

const EMPTY_RECENT_FILES: RecentFilesData = {
  game_paths: [],
  trainer_paths: [],
  dll_paths: [],
};

const PreferencesContext = createContext<PreferencesContextValue | null>(null);

function formatError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

async function loadPreferences() {
  const [loadedSettings, loadedRecentFiles, steamClientInstallPath] = await Promise.all([
    invoke<AppSettingsData>('settings_load'),
    invoke<RecentFilesData>('recent_files_load'),
    invoke<string>('default_steam_client_install_path'),
  ]);

  return {
    loadedSettings,
    loadedRecentFiles,
    steamClientInstallPath,
  };
}

export function PreferencesProvider({ children, activeProfileName }: PreferencesProviderProps) {
  const [settings, setSettings] = useState<AppSettingsData>(EMPTY_SETTINGS);
  const [recentFiles, setRecentFiles] = useState<RecentFilesData>(EMPTY_RECENT_FILES);
  const [settingsError, setSettingsError] = useState<string | null>(null);
  const [defaultSteamClientInstallPath, setDefaultSteamClientInstallPath] = useState('');

  const applyLoadedPreferences = useCallback((nextPreferences: Awaited<ReturnType<typeof loadPreferences>>) => {
    setSettings(nextPreferences.loadedSettings);
    setRecentFiles(nextPreferences.loadedRecentFiles);
    setDefaultSteamClientInstallPath(nextPreferences.steamClientInstallPath);
    setSettingsError(null);
  }, []);

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

  const handleAutoLoadChange = useCallback(
    async (enabled: boolean) => {
      const nextSettings = {
        ...settings,
        auto_load_last_profile: enabled,
        last_used_profile: activeProfileName?.trim() || settings.last_used_profile,
      } satisfies AppSettingsData;

      try {
        await invoke('settings_save', { data: nextSettings });
        setSettings(nextSettings);
        setSettingsError(null);
      } catch (error) {
        setSettingsError(formatError(error));
        throw error;
      }
    },
    [activeProfileName, settings]
  );

  const handleSteamGridDbApiKeyChange = useCallback(
    async (key: string) => {
      const trimmedKey = key.trim();
      const keyOrNull = trimmedKey.length > 0 ? trimmedKey : null;

      try {
        await invoke('settings_save_steamgriddb_key', { key: keyOrNull });
        setSettings((previous) => ({
          ...previous,
          has_steamgriddb_api_key: keyOrNull !== null,
        }));
        setSettingsError(null);
      } catch (error) {
        setSettingsError(formatError(error));
        throw error;
      }
    },
    []
  );

  const clearRecentFiles = useCallback(async () => {
    const nextRecentFiles = {
      game_paths: [],
      trainer_paths: [],
      dll_paths: [],
    } satisfies RecentFilesData;

    try {
      await invoke('recent_files_save', { data: nextRecentFiles });
      setRecentFiles(nextRecentFiles);
      setSettingsError(null);
    } catch (error) {
      setSettingsError(formatError(error));
      throw error;
    }
  }, []);

  const value = useMemo<PreferencesContextValue>(
    () => ({
      settings,
      recentFiles,
      settingsError,
      defaultSteamClientInstallPath,
      refreshPreferences,
      handleAutoLoadChange,
      handleSteamGridDbApiKeyChange,
      clearRecentFiles,
    }),
    [
      clearRecentFiles,
      defaultSteamClientInstallPath,
      handleAutoLoadChange,
      handleSteamGridDbApiKeyChange,
      recentFiles,
      refreshPreferences,
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
