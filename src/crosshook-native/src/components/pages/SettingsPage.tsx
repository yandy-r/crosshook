import SettingsPanel from '../SettingsPanel';
import { usePreferencesContext } from '../../context/PreferencesContext';
import { useProfileContext } from '../../context/ProfileContext';

const DEFAULT_PROFILES_DIRECTORY = '~/.config/crosshook/profiles';

export function SettingsPage() {
  const {
    settings,
    recentFiles,
    settingsError,
    refreshPreferences,
    handleAutoLoadChange,
    handleSteamGridDbApiKeyChange,
    clearRecentFiles,
  } = usePreferencesContext();
  const { targetHomePath, steamClientInstallPath } = useProfileContext();

  return (
    <div className="crosshook-page-scroll-shell crosshook-page-scroll-shell--fill crosshook-page-scroll-shell--settings">
      <div className="crosshook-route-stack crosshook-settings-page">
        {settingsError ? (
          <div className="crosshook-error-banner crosshook-error-banner--section" role="alert">
            {settingsError}
          </div>
        ) : null}

        <div className="crosshook-route-stack__body--fill crosshook-settings-page__body">
          <div className="crosshook-route-card-host">
            <div className="crosshook-route-card-scroll">
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
                recentFilesLimit={10}
                targetHomePath={targetHomePath}
                steamClientInstallPath={steamClientInstallPath}
                steamGridDbApiKey={settings.steamgriddb_api_key}
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
              />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export default SettingsPage;
