import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { ChangeEvent } from 'react';
import type { LauncherDeleteResult, LauncherInfo } from '../types';
import { CollapsibleSection } from './ui/CollapsibleSection';

interface RecentFilesState {
  gamePaths: string[];
  trainerPaths: string[];
  dllPaths: string[];
}

export interface SettingsPanelProps {
  autoLoadLastProfile: boolean;
  lastUsedProfile: string;
  profilesDirectoryPath: string;
  profilesDirectoryConfigured?: boolean;
  recentFiles: RecentFilesState;
  recentFilesLimit?: number;
  targetHomePath: string;
  steamClientInstallPath: string;
  onAutoLoadLastProfileChange: (enabled: boolean) => void;
  onProfilesDirectoryPathChange?: (path: string) => void;
  onRefreshRecentFiles?: () => void;
  onClearRecentFiles?: () => void;
}

function toDisplayList(paths: string[], maxItems?: number) {
  if (!Number.isFinite(maxItems) || !maxItems || maxItems <= 0) {
    return paths;
  }

  return paths.slice(0, maxItems);
}

function truncatePath(path: string) {
  const normalized = path.trim();
  if (normalized.length <= 96) {
    return normalized;
  }

  return `${normalized.slice(0, 40)}...${normalized.slice(-48)}`;
}

function RecentFilesSection({ label, paths, limit }: { label: string; paths: string[]; limit?: number }) {
  const visiblePaths = toDisplayList(paths, limit);
  const countSuffix =
    typeof limit === 'number' && limit > 0 && paths.length > limit
      ? ` showing ${limit} of ${paths.length}`
      : ` (${paths.length})`;

  return (
    <section className="crosshook-panel crosshook-settings-section">
      <div className="crosshook-settings-section-header">
        <div className="crosshook-heading-eyebrow">{label}</div>
        <div className="crosshook-muted crosshook-settings-meta">
          {paths.length === 0 ? 'No entries yet' : `Recent paths${countSuffix}`}
        </div>
      </div>

      {visiblePaths.length === 0 ? (
        <p className="crosshook-muted crosshook-settings-help">
          CrossHook will remember recently used {label.toLowerCase()} here once they are saved or loaded.
        </p>
      ) : (
        <ul className="crosshook-recent-list">
          {visiblePaths.map((path) => (
            <li key={path} className="crosshook-recent-item" title={path}>
              <div className="crosshook-recent-item__label">{truncatePath(path)}</div>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

function ManageLaunchersSection({
  targetHomePath,
  steamClientInstallPath,
}: {
  targetHomePath: string;
  steamClientInstallPath: string;
}) {
  const [launchers, setLaunchers] = useState<LauncherInfo[]>([]);
  const [deleting, setDeleting] = useState<string | null>(null);
  const [reexporting, setReexporting] = useState<string | null>(null);
  const [confirmSlug, setConfirmSlug] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const loadLaunchers = useCallback(async () => {
    try {
      const result = await invoke<LauncherInfo[]>('list_launchers', {
        targetHomePath,
        steamClientInstallPath,
      });
      setLaunchers(result);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, [targetHomePath, steamClientInstallPath]);

  useEffect(() => {
    void loadLaunchers();
  }, [loadLaunchers]);

  async function handleDelete(slug: string) {
    setDeleting(slug);
    setError(null);
    try {
      await invoke<LauncherDeleteResult>('delete_launcher_by_slug', {
        launcherSlug: slug,
        targetHomePath,
        steamClientInstallPath,
      });
      setConfirmSlug(null);
      await loadLaunchers();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setDeleting(null);
    }
  }

  async function handleReexport(slug: string) {
    setReexporting(slug);
    setError(null);
    try {
      await invoke('reexport_launcher_by_slug', {
        launcherSlug: slug,
        targetHomePath,
        steamClientInstallPath,
      });
      await loadLaunchers();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setReexporting(null);
    }
  }

  if (launchers.length === 0 && !error) {
    return null;
  }

  return (
    <CollapsibleSection
      title="Manage Launchers"
      defaultOpen={false}
      className="crosshook-panel crosshook-settings-section"
      meta={
        <>
          <span className="crosshook-muted">
            {launchers.length} launcher{launchers.length !== 1 ? 's' : ''} on disk
          </span>
          <button
            type="button"
            className="crosshook-button crosshook-button--ghost crosshook-button--ghost--small crosshook-settings-small-button"
            onClick={(event) => {
              event.preventDefault();
              void loadLaunchers();
            }}
          >
            Refresh
          </button>
        </>
      }
    >
      {error ? (
        <p className="crosshook-danger crosshook-settings-error">
          {error}
        </p>
      ) : null}

      <ul className="crosshook-recent-list">
        {launchers.map((launcher) => (
          <li key={launcher.launcher_slug} className="crosshook-recent-item">
            <div className="crosshook-settings-launcher-row">
              <div>
                <div className="crosshook-recent-item__label crosshook-settings-launcher-label">
                  {launcher.launcher_slug}
                  {launcher.is_stale ? (
                    <span className="crosshook-health-chip crosshook-health-chip--warning" style={{ marginLeft: 8 }}>
                      Stale
                    </span>
                  ) : null}
                </div>
                <div className="crosshook-recent-item__label crosshook-settings-launcher-path">
                  {launcher.script_exists ? truncatePath(launcher.script_path) : null}
                  {launcher.script_exists && launcher.desktop_entry_exists ? ' | ' : null}
                  {launcher.desktop_entry_exists ? truncatePath(launcher.desktop_entry_path) : null}
                </div>
              </div>
              <div className="crosshook-settings-launcher-actions">
                {launcher.is_stale ? (
                  <button
                    type="button"
                    className="crosshook-button crosshook-button--warning crosshook-settings-small-button"
                    disabled={reexporting === launcher.launcher_slug}
                    onClick={() => void handleReexport(launcher.launcher_slug)}
                  >
                    {reexporting === launcher.launcher_slug ? 'Re-exporting...' : 'Re-export'}
                  </button>
                ) : null}
                {confirmSlug === launcher.launcher_slug ? (
                  <>
                    <button
                      type="button"
                      className="crosshook-button crosshook-button--danger crosshook-settings-small-button"
                      disabled={deleting === launcher.launcher_slug || reexporting === launcher.launcher_slug}
                      onClick={() => void handleDelete(launcher.launcher_slug)}
                    >
                      {deleting === launcher.launcher_slug ? 'Deleting...' : 'Confirm'}
                    </button>
                    <button
                      type="button"
                      className="crosshook-button crosshook-button--ghost crosshook-button--ghost--small crosshook-settings-small-button"
                      onClick={() => setConfirmSlug(null)}
                    >
                      Cancel
                    </button>
                  </>
                ) : (
                  <button
                    type="button"
                    className="crosshook-button crosshook-button--ghost crosshook-button--ghost--small crosshook-settings-small-button"
                    onClick={() => setConfirmSlug(launcher.launcher_slug)}
                  >
                    Delete
                  </button>
                )}
              </div>
            </div>
          </li>
        ))}
      </ul>
    </CollapsibleSection>
  );
}

export function SettingsPanel({
  autoLoadLastProfile,
  lastUsedProfile,
  profilesDirectoryPath,
  profilesDirectoryConfigured = true,
  recentFiles,
  recentFilesLimit = 10,
  targetHomePath,
  steamClientInstallPath,
  onAutoLoadLastProfileChange,
  onProfilesDirectoryPathChange,
  onRefreshRecentFiles,
  onClearRecentFiles,
}: SettingsPanelProps) {
  const handleProfilesDirectoryChange = (event: ChangeEvent<HTMLInputElement>) => {
    onProfilesDirectoryPathChange?.(event.target.value);
  };

  const profilesDirectoryMessage = profilesDirectoryConfigured
    ? 'CrossHook will store profiles in the configured directory below.'
    : 'No custom directory is configured yet. CrossHook will use the default profile store until one is provided.';

  return (
    <section className="crosshook-card crosshook-settings-panel" aria-label="Settings">
      <header className="crosshook-settings-header">
        <div className="crosshook-heading-eyebrow">Settings</div>
        <h2 className="crosshook-heading-title">App preferences and storage</h2>
        <p className="crosshook-heading-copy">
          Keep startup behavior, profile storage, and recent file history in one place. The backend stores these values,
          and this panel reflects the current state for editing and review.
        </p>
      </header>

      <div className="crosshook-settings-summary">
        <span className="crosshook-status-chip">
          <strong>Last profile:</strong>
          <span>{lastUsedProfile.trim().length > 0 ? lastUsedProfile : 'none'}</span>
        </span>
        <span className="crosshook-status-chip">
          <strong>Recent limit:</strong>
          <span>{recentFilesLimit}</span>
        </span>
      </div>

      <div className="crosshook-settings-grid">
        <div className="crosshook-settings-column">
          <CollapsibleSection
            title="Startup"
            className="crosshook-panel crosshook-settings-section"
            meta={<span className="crosshook-muted">Controlled by settings.toml</span>}
          >
            <label className="crosshook-settings-checkbox-row">
              <input
                type="checkbox"
                checked={autoLoadLastProfile}
                onChange={(event) => onAutoLoadLastProfileChange(event.target.checked)}
                className="crosshook-settings-checkbox"
              />
              <span>
                <span className="crosshook-label">Auto-load last profile</span>
                <p className="crosshook-muted crosshook-settings-note">
                  When enabled, CrossHook should reopen the most recently used profile on startup if it still exists.
                </p>
              </span>
            </label>

            <div className="crosshook-settings-field-row">
              <label className="crosshook-label" htmlFor="last-used-profile">
                Last used profile
              </label>
              <input
                id="last-used-profile"
                className="crosshook-input"
                value={lastUsedProfile}
                readOnly
                placeholder="No profile selected"
              />
            </div>
          </CollapsibleSection>

          <CollapsibleSection
            title="Profiles"
            className="crosshook-panel crosshook-settings-section"
            meta={<span className="crosshook-muted">Storage location</span>}
          >
            <div className="crosshook-settings-field-row">
              <label className="crosshook-label" htmlFor="profiles-directory">
                Profiles directory
              </label>
              <div className="crosshook-settings-input-row">
                <input
                  id="profiles-directory"
                  className="crosshook-input"
                  value={profilesDirectoryPath}
                  onChange={handleProfilesDirectoryChange}
                  placeholder="~/.config/crosshook/profiles"
                  readOnly={!onProfilesDirectoryPathChange}
                />
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

            <p className="crosshook-muted crosshook-settings-help">
              {profilesDirectoryMessage}
            </p>
            <p className="crosshook-muted crosshook-settings-note">
              The native backend should resolve this to `~/.config/crosshook/profiles` by default and persist any custom
              location through the settings store.
            </p>

            {onClearRecentFiles ? (
              <div className="crosshook-settings-clear-row">
                <button type="button" className="crosshook-button crosshook-button--ghost" onClick={onClearRecentFiles}>
                  Clear recent history
                </button>
              </div>
            ) : null}
          </CollapsibleSection>

          <ManageLaunchersSection
            targetHomePath={targetHomePath}
            steamClientInstallPath={steamClientInstallPath}
          />
        </div>

        <section className="crosshook-settings-recent-column" aria-label="Recent files">
          <CollapsibleSection
            title="Recent Files"
            className="crosshook-panel crosshook-settings-section"
            meta={<span className="crosshook-muted">Most recent paths used by the app</span>}
          >
            <p className="crosshook-muted crosshook-settings-help">
              These lists are intended to come from the backend recent-files store. Non-existent entries should be
              removed before the data is passed into this component.
            </p>

            <RecentFilesSection label="Games" paths={recentFiles.gamePaths} limit={recentFilesLimit} />
            <RecentFilesSection label="Trainers" paths={recentFiles.trainerPaths} limit={recentFilesLimit} />
            <RecentFilesSection label="DLLs" paths={recentFiles.dllPaths} limit={recentFilesLimit} />
          </CollapsibleSection>
        </section>
      </div>
    </section>
  );
}

export default SettingsPanel;
