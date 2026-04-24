import { DashboardPanelSection } from './layout/DashboardPanelSection';
import { DiagnosticExportSection } from './settings/DiagnosticExportSection';
import { LoggingAndUiSection } from './settings/LoggingAndUiSection';
import { ManageLaunchersSection } from './settings/ManageLaunchersSection';
import { NewProfileDefaultsSection } from './settings/NewProfileDefaultsSection';
import { PrefixDependenciesSection } from './settings/PrefixDependenciesSection';
import { PrefixStorageHealthSection } from './settings/PrefixStorageHealthSection';
import { ProfilesSection } from './settings/ProfilesSection';
import { ProtonManagerDefaultsSection } from './settings/ProtonManagerDefaultsSection';
import { RecentFilesColumn } from './settings/RecentFilesColumn';
import { RunnerSection } from './settings/RunnerSection';
import { StartupSection } from './settings/StartupSection';
import { SteamGridDbSection } from './settings/SteamGridDbSection';
import type { SettingsPanelProps } from './settings/types';

export type { RecentFilesState, SettingsPanelProps } from './settings/types';

export function SettingsPanel({
  settings,
  onPersistSettings,
  recentFiles,
  targetHomePath,
  steamClientInstallPath,
  onAutoLoadLastProfileChange,
  onRefreshRecentFiles,
  onClearRecentFiles,
  onSteamGridDbApiKeyChange,
  onBrowseProfilesDirectory,
}: SettingsPanelProps) {
  const recentFilesLimit = settings.recent_files_limit;

  return (
    <DashboardPanelSection
      eyebrow="App"
      title="App preferences and storage"
      summary="Keep startup behavior, profile storage, and recent file history in one place. The backend stores these values, and this panel reflects the current state for editing and review."
      headingAfter={
        <div className="crosshook-settings-summary">
          <span className="crosshook-status-chip">
            <strong>Last profile:</strong>
            <span>{(settings.last_used_profile ?? '').trim().length > 0 ? settings.last_used_profile : 'none'}</span>
          </span>
          <span className="crosshook-status-chip">
            <strong>Recent limit:</strong>
            <span>{recentFilesLimit}</span>
          </span>
        </div>
      }
      className="crosshook-settings-panel"
    >
      <div className="crosshook-settings-grid">
        <div className="crosshook-settings-column">
          <StartupSection settings={settings} onAutoLoadLastProfileChange={onAutoLoadLastProfileChange} />

          <NewProfileDefaultsSection settings={settings} onPersistSettings={onPersistSettings} />

          <RunnerSection settings={settings} onPersistSettings={onPersistSettings} />

          <ProtonManagerDefaultsSection
            settings={settings}
            steamClientInstallPath={steamClientInstallPath}
            onPersistSettings={onPersistSettings}
          />

          <LoggingAndUiSection settings={settings} onPersistSettings={onPersistSettings} />

          <PrefixDependenciesSection settings={settings} onPersistSettings={onPersistSettings} />

          <ProfilesSection
            settings={settings}
            onPersistSettings={onPersistSettings}
            onRefreshRecentFiles={onRefreshRecentFiles}
            onClearRecentFiles={onClearRecentFiles}
            onBrowseProfilesDirectory={onBrowseProfilesDirectory}
          />

          <ManageLaunchersSection targetHomePath={targetHomePath} steamClientInstallPath={steamClientInstallPath} />

          <PrefixStorageHealthSection />

          <DiagnosticExportSection />

          <SteamGridDbSection hasApiKey={settings.has_steamgriddb_api_key} onApiKeyChange={onSteamGridDbApiKeyChange} />
        </div>

        <RecentFilesColumn recentFiles={recentFiles} recentFilesLimit={recentFilesLimit} />
      </div>
    </DashboardPanelSection>
  );
}

export default SettingsPanel;
