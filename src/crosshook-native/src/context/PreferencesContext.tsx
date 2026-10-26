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
  /** Optimistically updates `libraryViewMode` and persists via `settings_set_library_view_mode` (serialized; failure restores the last confirmed mode). */
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
  const latestViewModeRequestRef = useRef(0);
  const settledViewModeRequestRef = useRef(0);
  /** Last confirmed backend view mode (authoritative load or successful persist). */
  const lastConfirmedViewModeRef = useRef<LibraryViewMode | null>(null);

  // A load is authoritative only when no view-mode request started after it and
  // it started after the latest request settled. Older loads retain the current
  // optimistic mode, even when that request settles before the load finishes.
  const applyLoadedSettings = useCallback((loadedSettings: AppSettingsData, loadStartSettleSequence: number) => {
    const loadIsAuthoritative =
      loadStartSettleSequence === settledViewModeRequestRef.current &&
      latestViewModeRequestRef.current === settledViewModeRequestRef.current;
    if (loadIsAuthoritative) {
      pendingViewModeRef.current = null;
      setSettings(loadedSettings);
      // Authoritative loads confirm the backend value.
      lastConfirmedViewModeRef.current = loadedSettings.ui.library_view_mode ?? 'grid';
      return;
    }

    setSettings((previous) => ({
      ...loadedSettings,
      ui: {
        ...loadedSettings.ui,
        library_view_mode: pendingViewModeRef.current ?? previous.ui.library_view_mode,
      },
    }));
  }, []);

  const applyLoadedPreferences = useCallback(
    (nextPreferences: LoadedPreferences, loadStartSettleSequence: number) => {
      applyLoadedSettings(nextPreferences.loadedSettings, loadStartSettleSequence);
      setRecentFiles(nextPreferences.loadedRecentFiles);
      setDefaultSteamClientInstallPath(nextPreferences.steamClientInstallPath);
      setSettingsError(null);
    },
    [applyLoadedSettings]
  );

  const refreshPreferences = useCallback(async () => {
    const loadStartSettleSequence = settledViewModeRequestRef.current;
    try {
      applyLoadedPreferences(await loadPreferences(), loadStartSettleSequence);
    } catch (error) {
      setSettingsError(formatError(error));
      throw error;
    }
  }, [applyLoadedPreferences]);

  useEffect(() => {
    let active = true;

    async function initializePreferences() {
      try {
        const loadStartSettleSequence = settledViewModeRequestRef.current;
        const nextPreferences = await loadPreferences();
        if (!active) {
          return;
        }

        applyLoadedPreferences(nextPreferences, loadStartSettleSequence);
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
      const loadStartSettleSequence = settledViewModeRequestRef.current;
      const loaded = await callCommand<AppSettingsData>('settings_load');
      applyLoadedSettings(loaded, loadStartSettleSequence);
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

  // Promise queue serializing `settings_set_library_view_mode` so the backend
  // applies selections in user order even under rapid toggles.
  const viewModePersistQueueRef = useRef<Promise<void>>(Promise.resolve());

  const setLibraryViewMode = useCallback(
    (mode: LibraryViewMode) => {
      const requestSequence = latestViewModeRequestRef.current + 1;
      latestViewModeRequestRef.current = requestSequence;
      pendingViewModeRef.current = mode;
      setSettings((previous) => ({
        ...previous,
        ui: { ...previous.ui, library_view_mode: mode },
      }));
      const run = viewModePersistQueueRef.current
        .catch(() => undefined)
        .then(() => callCommand('settings_set_library_view_mode', { mode }));
      viewModePersistQueueRef.current = run.then(
        () => undefined,
        () => undefined
      );
      void run.then(
        () => {
          if (latestViewModeRequestRef.current !== requestSequence) {
            return;
          }
          // Retain the mode for loads that started before this settle. The next
          // authoritative load clears the guard regardless of the backend value.
          settledViewModeRequestRef.current = requestSequence;
          lastConfirmedViewModeRef.current = mode;
        },
        (error: unknown) => {
          console.error('Failed to persist library view mode', error);
          const restoreFallback = () => {
            // Restore the last confirmed mode only when no newer selection started.
            if (latestViewModeRequestRef.current !== requestSequence) {
              return;
            }
            const fallback = lastConfirmedViewModeRef.current;
            if (fallback === null || fallback === mode) {
              return;
            }
            pendingViewModeRef.current = null;
            setSettings((previous) => ({
              ...previous,
              ui: { ...previous.ui, library_view_mode: fallback },
            }));
          };
          // refreshPreferences records a recovery-load failure in settingsError.
          // A successful refresh is authoritative only when no load was already in
          // flight with stale data; without one (or on refresh failure), restore
          // the last confirmed mode. A superseded request only recovers via the
          // refresh when it is still awaiting its persist slot (settled === 0).
          const shouldRefresh =
            latestViewModeRequestRef.current === requestSequence || settledViewModeRequestRef.current === 0;
          if (shouldRefresh) {
            if (latestViewModeRequestRef.current === requestSequence) {
              settledViewModeRequestRef.current = requestSequence;
              pendingViewModeRef.current = null;
            }
            void refreshPreferences().then(restoreFallback, restoreFallback);
          } else {
            restoreFallback();
          }
        }
      );
    },
    [refreshPreferences]
  );

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
