import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { CSSProperties, ChangeEvent } from 'react';
import type { LauncherDeleteResult, LauncherInfo } from '../types';

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

const layoutStyles: Record<string, CSSProperties> = {
  root: {
    display: 'grid',
    gap: 20,
  },
  header: {
    display: 'grid',
    gap: 8,
  },
  summaryRow: {
    display: 'flex',
    flexWrap: 'wrap',
    gap: 10,
  },
  grid: {
    display: 'grid',
    gap: 20,
    gridTemplateColumns: 'minmax(0, 1fr) minmax(0, 1.1fr)',
    alignItems: 'start',
  },
  sectionGrid: {
    display: 'grid',
    gap: 16,
  },
  fieldRow: {
    display: 'grid',
    gap: 10,
  },
  inputRow: {
    display: 'flex',
    gap: 10,
    alignItems: 'center',
  },
  checkboxRow: {
    display: 'flex',
    alignItems: 'center',
    gap: 12,
  },
  checkbox: {
    width: 18,
    height: 18,
    minHeight: 18,
    accentColor: 'var(--crosshook-color-accent)',
    cursor: 'pointer',
  },
  helper: {
    margin: 0,
    color: 'var(--crosshook-color-text-muted)',
    lineHeight: 1.6,
  },
  note: {
    margin: 0,
    color: 'var(--crosshook-color-text-subtle)',
    fontSize: 13,
    lineHeight: 1.5,
  },
  recentColumn: {
    display: 'grid',
    gap: 12,
  },
  recentList: {
    display: 'grid',
    gap: 8,
    margin: 0,
    padding: 0,
    listStyle: 'none',
  },
  recentItem: {
    display: 'grid',
    gap: 4,
    padding: '10px 12px',
    borderRadius: 12,
    background: 'rgba(8, 14, 26, 0.78)',
    border: '1px solid var(--crosshook-color-border)',
  },
  recentItemLabel: {
    fontFamily: 'var(--crosshook-font-mono)',
    fontSize: 13,
    color: 'var(--crosshook-color-text)',
    wordBreak: 'break-word',
  },
  sectionHeader: {
    display: 'flex',
    justifyContent: 'space-between',
    gap: 12,
    alignItems: 'center',
    flexWrap: 'wrap',
  },
};

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
    <section className="crosshook-panel" style={layoutStyles.sectionGrid}>
      <div style={layoutStyles.sectionHeader}>
        <div className="crosshook-heading-eyebrow">{label}</div>
        <div className="crosshook-muted" style={{ fontSize: 13 }}>
          {paths.length === 0 ? 'No entries yet' : `Recent paths${countSuffix}`}
        </div>
      </div>

      {visiblePaths.length === 0 ? (
        <p className="crosshook-muted" style={layoutStyles.helper}>
          CrossHook will remember recently used {label.toLowerCase()} here once they are saved or loaded.
        </p>
      ) : (
        <ul style={layoutStyles.recentList}>
          {visiblePaths.map((path) => (
            <li key={path} style={layoutStyles.recentItem} title={path}>
              <div style={layoutStyles.recentItemLabel}>{truncatePath(path)}</div>
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
  const [expanded, setExpanded] = useState(false);
  const [deleting, setDeleting] = useState<string | null>(null);
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

  if (launchers.length === 0 && !error) {
    return null;
  }

  return (
    <section className="crosshook-panel" style={layoutStyles.sectionGrid}>
      <div style={layoutStyles.sectionHeader}>
        <div className="crosshook-heading-eyebrow">Manage Launchers</div>
        <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
          <div className="crosshook-muted" style={{ fontSize: 13 }}>
            {launchers.length} launcher{launchers.length !== 1 ? 's' : ''} on disk
          </div>
          <button
            type="button"
            className="crosshook-button crosshook-button--ghost"
            style={{ fontSize: 12, padding: '4px 8px' }}
            onClick={() => setExpanded((prev) => !prev)}
          >
            {expanded ? 'Collapse' : 'Expand'}
          </button>
          <button
            type="button"
            className="crosshook-button crosshook-button--ghost"
            style={{ fontSize: 12, padding: '4px 8px' }}
            onClick={() => void loadLaunchers()}
          >
            Refresh
          </button>
        </div>
      </div>

      {error ? (
        <p className="crosshook-danger" style={{ margin: 0, fontSize: 13 }}>
          {error}
        </p>
      ) : null}

      {expanded ? (
        <ul style={layoutStyles.recentList}>
          {launchers.map((launcher) => (
            <li key={launcher.launcher_slug} style={layoutStyles.recentItem}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 8 }}>
                <div>
                  <div style={{ ...layoutStyles.recentItemLabel, fontWeight: 600 }}>
                    {launcher.launcher_slug}
                  </div>
                  <div style={{ ...layoutStyles.recentItemLabel, fontSize: 12, color: 'var(--crosshook-color-text-muted)' }}>
                    {launcher.script_exists ? truncatePath(launcher.script_path) : null}
                    {launcher.script_exists && launcher.desktop_entry_exists ? ' | ' : null}
                    {launcher.desktop_entry_exists ? truncatePath(launcher.desktop_entry_path) : null}
                  </div>
                </div>
                <div style={{ display: 'flex', gap: 6, flexShrink: 0 }}>
                  {confirmSlug === launcher.launcher_slug ? (
                    <>
                      <button
                        type="button"
                        className="crosshook-button crosshook-button--danger"
                        style={{ fontSize: 12, padding: '4px 10px' }}
                        disabled={deleting === launcher.launcher_slug}
                        onClick={() => void handleDelete(launcher.launcher_slug)}
                      >
                        {deleting === launcher.launcher_slug ? 'Deleting...' : 'Confirm'}
                      </button>
                      <button
                        type="button"
                        className="crosshook-button crosshook-button--ghost"
                        style={{ fontSize: 12, padding: '4px 10px' }}
                        onClick={() => setConfirmSlug(null)}
                      >
                        Cancel
                      </button>
                    </>
                  ) : (
                    <button
                      type="button"
                      className="crosshook-button crosshook-button--ghost"
                      style={{ fontSize: 12, padding: '4px 10px' }}
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
      ) : null}
    </section>
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
    <section className="crosshook-card" aria-label="Settings" style={layoutStyles.root}>
      <header style={layoutStyles.header}>
        <div className="crosshook-heading-eyebrow">Settings</div>
        <h2 className="crosshook-heading-title">App preferences and storage</h2>
        <p className="crosshook-heading-copy">
          Keep startup behavior, profile storage, and recent file history in one place. The backend stores these values,
          and this panel reflects the current state for editing and review.
        </p>
      </header>

      <div style={layoutStyles.summaryRow}>
        <span className="crosshook-status-chip">
          <strong>Last profile:</strong>
          <span>{lastUsedProfile.trim().length > 0 ? lastUsedProfile : 'none'}</span>
        </span>
        <span className="crosshook-status-chip">
          <strong>Recent limit:</strong>
          <span>{recentFilesLimit}</span>
        </span>
      </div>

      <div style={layoutStyles.grid}>
        <div style={layoutStyles.sectionGrid}>
          <section className="crosshook-panel" style={layoutStyles.sectionGrid}>
            <div style={layoutStyles.sectionHeader}>
              <div className="crosshook-heading-eyebrow">Startup</div>
              <div className="crosshook-muted" style={{ fontSize: 13 }}>
                Controlled by settings.toml
              </div>
            </div>

            <label style={layoutStyles.checkboxRow}>
              <input
                type="checkbox"
                checked={autoLoadLastProfile}
                onChange={(event) => onAutoLoadLastProfileChange(event.target.checked)}
                style={layoutStyles.checkbox}
              />
              <span>
                <span className="crosshook-label">Auto-load last profile</span>
                <p className="crosshook-muted" style={layoutStyles.note}>
                  When enabled, CrossHook should reopen the most recently used profile on startup if it still exists.
                </p>
              </span>
            </label>

            <div style={layoutStyles.fieldRow}>
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
          </section>

          <section className="crosshook-panel" style={layoutStyles.sectionGrid}>
            <div style={layoutStyles.sectionHeader}>
              <div className="crosshook-heading-eyebrow">Profiles</div>
              <div className="crosshook-muted" style={{ fontSize: 13 }}>
                Storage location
              </div>
            </div>

            <div style={layoutStyles.fieldRow}>
              <label className="crosshook-label" htmlFor="profiles-directory">
                Profiles directory
              </label>
              <div style={layoutStyles.inputRow}>
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

            <p className="crosshook-muted" style={layoutStyles.helper}>
              {profilesDirectoryMessage}
            </p>
            <p className="crosshook-muted" style={layoutStyles.note}>
              The native backend should resolve this to `~/.config/crosshook/profiles` by default and persist any custom
              location through the settings store.
            </p>

            {onClearRecentFiles ? (
              <div style={{ display: 'flex', justifyContent: 'flex-end' }}>
                <button type="button" className="crosshook-button crosshook-button--ghost" onClick={onClearRecentFiles}>
                  Clear recent history
                </button>
              </div>
            ) : null}
          </section>

          <ManageLaunchersSection
            targetHomePath={targetHomePath}
            steamClientInstallPath={steamClientInstallPath}
          />
        </div>

        <section style={layoutStyles.recentColumn} aria-label="Recent files">
          <div className="crosshook-panel" style={layoutStyles.sectionGrid}>
            <div style={layoutStyles.sectionHeader}>
              <div className="crosshook-heading-eyebrow">Recent Files</div>
              <div className="crosshook-muted" style={{ fontSize: 13 }}>
                Most recent paths used by the app
              </div>
            </div>
            <p className="crosshook-muted" style={layoutStyles.helper}>
              These lists are intended to come from the backend recent-files store. Non-existent entries should be
              removed before the data is passed into this component.
            </p>
          </div>

          <RecentFilesSection label="Games" paths={recentFiles.gamePaths} limit={recentFilesLimit} />
          <RecentFilesSection label="Trainers" paths={recentFiles.trainerPaths} limit={recentFilesLimit} />
          <RecentFilesSection label="DLLs" paths={recentFiles.dllPaths} limit={recentFilesLimit} />
        </section>
      </div>
    </section>
  );
}

export default SettingsPanel;
