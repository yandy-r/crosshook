import { useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import ConsoleView from './components/ConsoleView';
import LaunchPanel from './components/LaunchPanel';
import { ProfileEditorView } from './components/ProfileEditor';
import { SettingsPanel } from './components/SettingsPanel';
import { useGamepadNav } from './hooks/useGamepadNav';
import { useProfile } from './hooks/useProfile';
import type { AppSettingsData, RecentFilesData, SteamLaunchRequest } from './types';

const DEFAULT_SETTINGS: AppSettingsData = {
  auto_load_last_profile: false,
  last_used_profile: '',
};

const DEFAULT_RECENT_FILES: RecentFilesData = {
  game_paths: [],
  trainer_paths: [],
  dll_paths: [],
};

const DEFAULT_PROFILES_DIRECTORY = '~/.config/crosshook/profiles';

function deriveSteamClientInstallPath(compatdataPath: string): string {
  const marker = '/steamapps/compatdata/';
  const normalized = compatdataPath.trim().replace(/\\/g, '/');
  const index = normalized.indexOf(marker);

  return index >= 0 ? normalized.slice(0, index) : '';
}

export function App() {
  const profileState = useProfile({ autoSelectFirstProfile: false });
  const { profile, profileName, selectProfile } = profileState;
  const [settings, setSettings] = useState<AppSettingsData>(DEFAULT_SETTINGS);
  const [recentFiles, setRecentFiles] = useState<RecentFilesData>(DEFAULT_RECENT_FILES);
  const [settingsError, setSettingsError] = useState<string | null>(null);
  const gamepadNav = useGamepadNav();

  const launchRequest = useMemo<SteamLaunchRequest | null>(() => {
    if (!profile.steam.enabled) {
      return null;
    }

    return {
      game_path: profile.game.executable_path,
      trainer_path: profile.trainer.path,
      trainer_host_path: profile.trainer.path,
      steam_app_id: profile.steam.app_id,
      steam_compat_data_path: profile.steam.compatdata_path,
      steam_proton_path: profile.steam.proton_path,
      steam_client_install_path: deriveSteamClientInstallPath(profile.steam.compatdata_path),
      launch_trainer_only: false,
      launch_game_only: false,
    };
  }, [profile]);

  useEffect(() => {
    let active = true;

    async function loadPreferences() {
      try {
        const [loadedSettings, loadedRecentFiles] = await Promise.all([
          invoke<AppSettingsData>('settings_load'),
          invoke<RecentFilesData>('recent_files_load'),
        ]);

        if (!active) {
          return;
        }

        setSettings(loadedSettings);
        setRecentFiles(loadedRecentFiles);
        setSettingsError(null);
      } catch (error) {
        if (active) {
          setSettingsError(error instanceof Error ? error.message : String(error));
        }
      }
    }

    void loadPreferences();

    const unlistenPromise = listen<string>('auto-load-profile', (event) => {
      void selectProfile(event.payload);
    });

    return () => {
      active = false;
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, [selectProfile]);

  async function refreshPreferences() {
    const [loadedSettings, loadedRecentFiles] = await Promise.all([
      invoke<AppSettingsData>('settings_load'),
      invoke<RecentFilesData>('recent_files_load'),
    ]);
    setSettings(loadedSettings);
    setRecentFiles(loadedRecentFiles);
    setSettingsError(null);
  }

  async function handleAutoLoadChange(enabled: boolean) {
    const nextSettings = {
      ...settings,
      auto_load_last_profile: enabled,
      last_used_profile: profileName.trim() || settings.last_used_profile,
    } satisfies AppSettingsData;

    await invoke('settings_save', { data: nextSettings });
    setSettings(nextSettings);
  }

  async function clearRecentFiles() {
    const nextRecentFiles = {
      game_paths: [],
      trainer_paths: [],
      dll_paths: [],
    } satisfies RecentFilesData;

    await invoke('recent_files_save', { data: nextRecentFiles });
    setRecentFiles(nextRecentFiles);
  }

  return (
    <main ref={gamepadNav.rootRef} className="crosshook-app crosshook-focus-scope">
      <div className="crosshook-shell">
        <header style={{ display: 'grid', gap: '8px' }}>
          <div className="crosshook-heading-eyebrow">CrossHook Native</div>
          <h1 className="crosshook-heading-title">Two-step Steam launch</h1>
          <p className="crosshook-heading-copy">
            Launch the game first, then switch to trainer mode once the game reaches the main menu. The console below
            streams helper output.
          </p>
          <div style={{ display: 'flex', gap: 12, flexWrap: 'wrap' }}>
            <span className="crosshook-status-chip">Controller mode: {gamepadNav.controllerMode ? 'On' : 'Off'}</span>
            <span className="crosshook-status-chip">Last profile: {settings.last_used_profile || 'none'}</span>
          </div>
          {settingsError ? (
            <p className="crosshook-danger" style={{ margin: 0 }}>
              {settingsError}
            </p>
          ) : null}
        </header>

        <div className="crosshook-layout">
          <div className="stack">
            <ProfileEditorView state={profileState} />
            <SettingsPanel
              autoLoadLastProfile={settings.auto_load_last_profile}
              lastUsedProfile={settings.last_used_profile}
              profilesDirectoryPath={DEFAULT_PROFILES_DIRECTORY}
              profilesDirectoryConfigured={false}
              recentFiles={{
                gamePaths: recentFiles.game_paths,
                trainerPaths: recentFiles.trainer_paths,
                dllPaths: recentFiles.dll_paths,
              }}
              onAutoLoadLastProfileChange={(enabled) => {
                void handleAutoLoadChange(enabled).catch((error) => {
                  setSettingsError(error instanceof Error ? error.message : String(error));
                });
              }}
              onRefreshRecentFiles={() => {
                void refreshPreferences().catch((error) => {
                  setSettingsError(error instanceof Error ? error.message : String(error));
                });
              }}
              onClearRecentFiles={() => {
                void clearRecentFiles().catch((error) => {
                  setSettingsError(error instanceof Error ? error.message : String(error));
                });
              }}
            />
          </div>
          <div className="stack">
            <LaunchPanel
              profileId={profileName || 'new-profile'}
              steamModeEnabled={profile.steam.enabled}
              request={launchRequest}
            />
            <ConsoleView />
          </div>
        </div>
      </div>
    </main>
  );
}

export default App;
