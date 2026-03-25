import { useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import ConsoleView from './components/ConsoleView';
import CommunityBrowser from './components/CommunityBrowser';
import CompatibilityViewer from './components/CompatibilityViewer';
import LaunchPanel from './components/LaunchPanel';
import LauncherExport from './components/LauncherExport';
import { ProfileEditorView } from './components/ProfileEditor';
import { SettingsPanel } from './components/SettingsPanel';
import { useCommunityProfiles } from './hooks/useCommunityProfiles';
import { useGamepadNav } from './hooks/useGamepadNav';
import { useProfile } from './hooks/useProfile';
import type { AppSettingsData, GameProfile, LaunchMethod, LaunchRequest, RecentFilesData } from './types';

type AppTab = 'main' | 'settings' | 'community';

const DEFAULT_SETTINGS: AppSettingsData = {
  auto_load_last_profile: false,
  last_used_profile: '',
  community_taps: [],
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

function deriveTargetHomePath(steamClientInstallPath: string): string {
  const normalized = steamClientInstallPath.trim().replace(/\\/g, '/');

  for (const suffix of ['/.local/share/Steam', '/.steam/root']) {
    if (normalized.endsWith(suffix)) {
      return normalized.slice(0, -suffix.length);
    }
  }

  return '';
}

function resolveLaunchMethod(profile: GameProfile): Exclude<LaunchMethod, ''> {
  const method = profile.launch.method.trim();

  if (method === 'steam_applaunch' || method === 'proton_run' || method === 'native') {
    return method;
  }

  if (profile.steam.enabled) {
    return 'steam_applaunch';
  }

  if (profile.game.executable_path.trim().toLowerCase().endsWith('.exe')) {
    return 'proton_run';
  }

  return 'native';
}

export function App() {
  const profileState = useProfile({ autoSelectFirstProfile: false });
  const { profile, profileName, selectProfile } = profileState;
  const [settings, setSettings] = useState<AppSettingsData>(DEFAULT_SETTINGS);
  const [recentFiles, setRecentFiles] = useState<RecentFilesData>(DEFAULT_RECENT_FILES);
  const [settingsError, setSettingsError] = useState<string | null>(null);
  const [defaultSteamClientInstallPath, setDefaultSteamClientInstallPath] = useState('');
  const [activeTab, setActiveTab] = useState<AppTab>('main');
  const [profileEditorTab, setProfileEditorTab] = useState<'profile' | 'install'>('profile');
  const gamepadNav = useGamepadNav();
  const communityState = useCommunityProfiles({
    profilesDirectoryPath: DEFAULT_PROFILES_DIRECTORY,
  });
  const launchMethod = useMemo(() => resolveLaunchMethod(profile), [profile]);
  const effectiveLaunchMethod = useMemo<Exclude<LaunchMethod, ''>>(() => {
    if (activeTab === 'main' && profileEditorTab === 'install') {
      return 'proton_run';
    }

    return launchMethod;
  }, [activeTab, launchMethod, profileEditorTab]);
  const steamClientInstallPath = useMemo(() => {
    return defaultSteamClientInstallPath || deriveSteamClientInstallPath(profile.steam.compatdata_path);
  }, [defaultSteamClientInstallPath, profile.steam.compatdata_path]);
  const targetHomePath = useMemo(() => deriveTargetHomePath(steamClientInstallPath), [steamClientInstallPath]);
  const shouldShowLauncherExport =
    profileEditorTab === 'install' ||
    effectiveLaunchMethod === 'steam_applaunch' ||
    effectiveLaunchMethod === 'proton_run';

  const launchRequest = useMemo<LaunchRequest | null>(() => {
    if (!profile.game.executable_path.trim()) {
      return null;
    }

    return {
      method: effectiveLaunchMethod,
      game_path: profile.game.executable_path,
      trainer_path: profile.trainer.path,
      trainer_host_path: profile.trainer.path,
      steam: {
        app_id: profile.steam.app_id,
        compatdata_path: profile.steam.compatdata_path,
        proton_path: profile.steam.proton_path,
        steam_client_install_path: steamClientInstallPath,
      },
      runtime: {
        prefix_path: profile.runtime.prefix_path,
        proton_path: profile.runtime.proton_path,
        working_directory: profile.runtime.working_directory,
      },
      launch_trainer_only: false,
      launch_game_only: false,
    };
  }, [effectiveLaunchMethod, profile, steamClientInstallPath]);

  const headingTitle = (() => {
      switch (effectiveLaunchMethod) {
      case 'steam_applaunch':
        return 'Two-step Steam launch';
      case 'proton_run':
        return 'Two-step Proton launch';
      case 'native':
      default:
        return 'Native launch';
    }
  })();

  const headingCopy = (() => {
      switch (effectiveLaunchMethod) {
      case 'steam_applaunch':
        return 'Launch the game through Steam first, then switch to trainer mode once the game reaches the main menu.';
      case 'proton_run':
        return 'Launch the game through Proton first, then launch the trainer into the same configured prefix.';
      case 'native':
      default:
        return 'Launch a Linux-native executable directly without Steam or Proton runner requirements.';
    }
  })();

  const compatibilityEntries = useMemo(
    () =>
      communityState.index.entries.map((entry) => ({
        id: `${entry.tap_url}::${entry.relative_path}`,
        tap_url: entry.tap_url,
        tap_branch: entry.tap_branch,
        manifest_path: entry.manifest_path,
        relative_path: entry.relative_path,
        metadata: entry.manifest.metadata,
      })),
    [communityState.index.entries]
  );

  useEffect(() => {
    let active = true;

    async function loadPreferences() {
      try {
        const [loadedSettings, loadedRecentFiles, steamClientPath] = await Promise.all([
          invoke<AppSettingsData>('settings_load'),
          invoke<RecentFilesData>('recent_files_load'),
          invoke<string>('default_steam_client_install_path'),
        ]);

        if (!active) {
          return;
        }

        setSettings(loadedSettings);
        setRecentFiles(loadedRecentFiles);
        setDefaultSteamClientInstallPath(steamClientPath);
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
          <h1 className="crosshook-heading-title">{headingTitle}</h1>
          <p className="crosshook-heading-copy">
            {headingCopy} The console below streams launcher output when a runner writes logs.
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
        <div className="crosshook-tab-row" role="tablist" aria-label="CrossHook sections">
          <button
            type="button"
            className={`crosshook-tab ${activeTab === 'main' ? 'crosshook-tab--active' : ''}`}
            onClick={() => setActiveTab('main')}
          >
            Main
          </button>
          <button
            type="button"
            className={`crosshook-tab ${activeTab === 'settings' ? 'crosshook-tab--active' : ''}`}
            onClick={() => setActiveTab('settings')}
          >
            Settings
          </button>
          <button
            type="button"
            className={`crosshook-tab ${activeTab === 'community' ? 'crosshook-tab--active' : ''}`}
            onClick={() => setActiveTab('community')}
          >
            Community
          </button>
        </div>

        {activeTab === 'main' ? (
          <div style={{ display: 'grid', gap: '24px' }}>
            <div className="crosshook-layout" style={{ alignItems: 'stretch' }}>
              <div style={{ display: 'grid', gap: '24px' }}>
                <ProfileEditorView state={profileState} onEditorTabChange={setProfileEditorTab} />
              </div>
              <div
                style={{
                  display: 'grid',
                  gap: '24px',
                  height: '100%',
                  minHeight: 0,
                  gridTemplateRows: shouldShowLauncherExport ? 'repeat(2, minmax(0, 1fr))' : undefined,
                }}
              >
                <LaunchPanel
                  profileId={profileName || 'new-profile'}
                  method={effectiveLaunchMethod}
                  request={profileEditorTab === 'install' ? null : launchRequest}
                  context={profileEditorTab === 'install' ? 'install' : 'default'}
                />
                {shouldShowLauncherExport ? (
                  <LauncherExport
                    profile={profile}
                    method={effectiveLaunchMethod}
                    steamClientInstallPath={steamClientInstallPath}
                    targetHomePath={targetHomePath}
                    context={profileEditorTab === 'install' ? 'install' : 'default'}
                  />
                ) : null}
              </div>
            </div>
            <ConsoleView />
          </div>
        ) : null}

        {activeTab === 'settings' ? (
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
            targetHomePath={targetHomePath}
            steamClientInstallPath={steamClientInstallPath}
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
        ) : null}

        {activeTab === 'community' ? (
          <div className="stack">
            <CommunityBrowser profilesDirectoryPath={DEFAULT_PROFILES_DIRECTORY} state={communityState} />
            <CompatibilityViewer
              entries={compatibilityEntries}
              loading={communityState.loading || communityState.syncing}
              error={communityState.error}
              emptyMessage="No indexed community compatibility entries are available yet."
            />
          </div>
        ) : null}
      </div>
    </main>
  );
}

export default App;
