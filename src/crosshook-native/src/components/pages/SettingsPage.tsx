import SettingsPanel from '../SettingsPanel';
import { PageBanner, SettingsArt } from '../layout/PageBanner';
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
    clearRecentFiles,
  } = usePreferencesContext();
  const { targetHomePath, steamClientInstallPath } = useProfileContext();

  return (
    <>
      <PageBanner
        eyebrow="Settings"
        title="App preferences and storage"
        copy="Manage startup behavior, recent file history, and storage-related defaults from one page."
        illustration={<SettingsArt />}
      />

      {settingsError ? (
        <div className="crosshook-error-banner crosshook-error-banner--section" role="alert">
          {settingsError}
        </div>
      ) : null}

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
        onAutoLoadLastProfileChange={(enabled) => {
          void handleAutoLoadChange(enabled);
        }}
        onRefreshRecentFiles={() => {
          void refreshPreferences();
        }}
        onClearRecentFiles={() => {
          void clearRecentFiles();
        }}
      />
    </>
  );
}

export default SettingsPage;
