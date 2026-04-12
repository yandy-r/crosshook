import { useCallback, useMemo } from 'react';
import { usePreferencesContext } from '../../context/PreferencesContext';
import { useProfileContext } from '../../context/ProfileContext';
import { chooseDirectory } from '../../utils/dialog';
import { deriveTargetHomePath } from '../../utils/steam';
import { RouteBanner } from '../layout/RouteBanner';
import SettingsPanel from '../SettingsPanel';

export function SettingsPage() {
  const {
    settings,
    recentFiles,
    settingsError,
    defaultSteamClientInstallPath,
    refreshPreferences,
    persistSettings,
    handleAutoLoadChange,
    handleSteamGridDbApiKeyChange,
    clearRecentFiles,
  } = usePreferencesContext();
  const { steamClientInstallPath: profileSteamPath } = useProfileContext();
  const effectiveSteamClientInstallPath = useMemo(
    () => defaultSteamClientInstallPath || profileSteamPath,
    [defaultSteamClientInstallPath, profileSteamPath]
  );
  const targetHomePath = useMemo(
    () => deriveTargetHomePath(effectiveSteamClientInstallPath),
    [effectiveSteamClientInstallPath]
  );

  const handleBrowseProfilesDirectory = useCallback(async () => {
    const dir = await chooseDirectory('Select profiles directory');
    if (dir?.trim()) {
      await persistSettings({ profiles_directory: dir.trim() });
    }
  }, [persistSettings]);

  return (
    <div className="crosshook-page-scroll-shell crosshook-page-scroll-shell--fill crosshook-page-scroll-shell--settings">
      <div className="crosshook-route-stack crosshook-settings-page">
        {settingsError ? (
          <div className="crosshook-error-banner crosshook-error-banner--section" role="alert">
            {settingsError}
          </div>
        ) : null}

        <div className="crosshook-route-stack__body--fill crosshook-settings-page__body">
          <RouteBanner route="settings" />
          <div className="crosshook-route-card-host">
            <div className="crosshook-route-card-scroll">
              <SettingsPanel
                settings={settings}
                onPersistSettings={persistSettings}
                recentFiles={{
                  gamePaths: recentFiles.game_paths,
                  trainerPaths: recentFiles.trainer_paths,
                  dllPaths: recentFiles.dll_paths,
                }}
                targetHomePath={targetHomePath}
                steamClientInstallPath={effectiveSteamClientInstallPath}
                onAutoLoadLastProfileChange={(enabled) => {
                  void handleAutoLoadChange(enabled);
                }}
                onRefreshRecentFiles={() => {
                  void refreshPreferences();
                }}
                onClearRecentFiles={() => {
                  void clearRecentFiles();
                }}
                onSteamGridDbApiKeyChange={handleSteamGridDbApiKeyChange}
                onBrowseProfilesDirectory={handleBrowseProfilesDirectory}
              />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export default SettingsPage;
