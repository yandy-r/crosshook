import type { AppSettingsData } from '../../types';
import { CollapsibleSection } from '../ui/CollapsibleSection';

interface ProfilesSectionProps {
  settings: AppSettingsData;
  onPersistSettings: (patch: Partial<AppSettingsData>) => Promise<void>;
  onRefreshRecentFiles?: () => void;
  onClearRecentFiles?: () => void;
  onBrowseProfilesDirectory?: () => void;
}

/** Collapsible section for configuring the profiles storage directory. */
export function ProfilesSection({
  settings,
  onPersistSettings,
  onRefreshRecentFiles,
  onClearRecentFiles,
  onBrowseProfilesDirectory,
}: ProfilesSectionProps) {
  const profilesDirectoryMessage = settings.profiles_directory.trim()
    ? 'Custom path is saved in settings.toml. Restart CrossHook to use it as the active profile store.'
    : 'Leave empty to use the default directory under your CrossHook config folder.';

  return (
    <CollapsibleSection
      title="Profiles"
      defaultOpen={false}
      className="crosshook-panel crosshook-settings-section"
      meta={<span className="crosshook-muted">Storage location</span>}
    >
      <div className="crosshook-settings-field-row">
        <label className="crosshook-label" htmlFor="profiles-directory">
          Profiles directory override
        </label>
        <div className="crosshook-settings-input-row">
          <input
            id="profiles-directory"
            key={`pd-${settings.profiles_directory}`}
            className="crosshook-input"
            defaultValue={settings.profiles_directory}
            placeholder="Empty = default (~/.config/crosshook/profiles)"
            onBlur={(event) => {
              const v = event.target.value.trim();
              if (v !== settings.profiles_directory.trim()) {
                void onPersistSettings({ profiles_directory: v });
              }
            }}
          />
          {onBrowseProfilesDirectory ? (
            <button
              type="button"
              className="crosshook-button crosshook-button--secondary"
              onClick={() => void onBrowseProfilesDirectory()}
            >
              Browse…
            </button>
          ) : null}
          {onRefreshRecentFiles ? (
            <button
              type="button"
              className="crosshook-button crosshook-button--secondary"
              onClick={onRefreshRecentFiles}
            >
              Refresh
            </button>
          ) : null}
        </div>
      </div>

      <p className="crosshook-muted crosshook-settings-help">{profilesDirectoryMessage}</p>
      <p className="crosshook-muted crosshook-settings-note">
        <strong>Active (this session):</strong> {settings.active_profiles_directory || '—'}
        <br />
        <strong>Resolved from settings:</strong> {settings.resolved_profiles_directory || '—'}
      </p>
      {settings.profiles_directory_requires_restart ? (
        <p className="crosshook-warning-banner crosshook-settings-help" role="status">
          Restart CrossHook to use the resolved profiles directory as the active store.
        </p>
      ) : null}

      {onClearRecentFiles ? (
        <div className="crosshook-settings-clear-row">
          <button type="button" className="crosshook-button crosshook-button--ghost" onClick={onClearRecentFiles}>
            Clear recent history
          </button>
        </div>
      ) : null}
    </CollapsibleSection>
  );
}
