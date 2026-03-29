import { useEffect, useState } from 'react';
import type { ChangeEvent } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useLauncherManagement } from '../hooks/useLauncherManagement';
import { chooseDirectory } from '../utils/dialog';
import { CollapsibleSection } from './ui/CollapsibleSection';
import type { DiagnosticBundleResult } from '../types';

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
  const [confirmSlug, setConfirmSlug] = useState<string | null>(null);
  const {
    launchers,
    error,
    isListing,
    deletingSlug,
    reexportingSlug,
    listLaunchers,
    deleteLauncher,
    reexportLauncher,
  } = useLauncherManagement({
    targetHomePath,
    steamClientInstallPath,
  });

  useEffect(() => {
    void listLaunchers();
  }, [listLaunchers]);

  async function handleDelete(slug: string) {
    const deleted = await deleteLauncher(slug);
    if (deleted) {
      setConfirmSlug(null);
    }
  }

  async function handleReexport(slug: string) {
    await reexportLauncher(slug);
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
              void listLaunchers();
            }}
          >
            {isListing ? 'Refreshing...' : 'Refresh'}
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
                    disabled={reexportingSlug === launcher.launcher_slug}
                    onClick={() => void handleReexport(launcher.launcher_slug)}
                  >
                    {reexportingSlug === launcher.launcher_slug ? 'Re-exporting...' : 'Re-export'}
                  </button>
                ) : null}
                {confirmSlug === launcher.launcher_slug ? (
                  <>
                    <button
                      type="button"
                      className="crosshook-button crosshook-button--danger crosshook-settings-small-button"
                      disabled={deletingSlug === launcher.launcher_slug || reexportingSlug === launcher.launcher_slug}
                      onClick={() => void handleDelete(launcher.launcher_slug)}
                    >
                      {deletingSlug === launcher.launcher_slug ? 'Deleting...' : 'Confirm'}
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

function DiagnosticExportSection() {
  const [isExporting, setIsExporting] = useState(false);
  const [redactPaths, setRedactPaths] = useState(true);
  const [useDefaultLocation, setUseDefaultLocation] = useState(true);
  const [customDir, setCustomDir] = useState<string | null>(null);
  const [result, setResult] = useState<DiagnosticBundleResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function handleBrowse() {
    const selected = await chooseDirectory('Choose export location');
    if (selected) {
      setCustomDir(selected);
    }
  }

  async function handleExport() {
    const outputDir = useDefaultLocation ? null : customDir;
    if (!useDefaultLocation && !outputDir) {
      setError('Choose a directory first or use the default location.');
      return;
    }

    setIsExporting(true);
    setError(null);
    setResult(null);
    try {
      const bundleResult = await invoke<DiagnosticBundleResult>('export_diagnostics', {
        redactPaths,
        outputDir,
      });
      setResult(bundleResult);
    } catch (err) {
      setError(String(err));
    } finally {
      setIsExporting(false);
    }
  }

  return (
    <CollapsibleSection
      title="Diagnostic Export"
      defaultOpen={false}
      className="crosshook-panel crosshook-settings-section"
      meta={<span className="crosshook-muted">Bug reports and troubleshooting</span>}
    >
      <p className="crosshook-muted crosshook-settings-help">
        Export a diagnostic bundle containing system info, profiles, logs, and Steam diagnostics as a
        single .tar.gz archive. Attach this to GitHub issues for faster troubleshooting.
      </p>

      <label className="crosshook-settings-checkbox-row">
        <input
          type="checkbox"
          checked={redactPaths}
          onChange={(event) => setRedactPaths(event.target.checked)}
          className="crosshook-settings-checkbox"
        />
        <span>
          <span className="crosshook-label">Redact home directory paths</span>
          <p className="crosshook-muted crosshook-settings-note">
            Replaces your home directory with ~ in profile configs and settings before bundling.
          </p>
        </span>
      </label>

      <label className="crosshook-settings-checkbox-row">
        <input
          type="checkbox"
          checked={useDefaultLocation}
          onChange={(event) => setUseDefaultLocation(event.target.checked)}
          className="crosshook-settings-checkbox"
        />
        <span>
          <span className="crosshook-label">Use default location</span>
          <p className="crosshook-muted crosshook-settings-note">
            Save the bundle to the system temp directory.
          </p>
        </span>
      </label>

      {!useDefaultLocation ? (
        <div className="crosshook-settings-field-row">
          <label className="crosshook-label">Export directory</label>
          <div className="crosshook-settings-input-row">
            <input
              className="crosshook-input"
              value={customDir ?? ''}
              readOnly
              placeholder="No directory selected"
            />
            <button
              type="button"
              className="crosshook-button crosshook-button--secondary"
              onClick={() => void handleBrowse()}
            >
              Browse
            </button>
          </div>
        </div>
      ) : null}

      <div className="crosshook-settings-clear-row">
        <button
          type="button"
          className="crosshook-button crosshook-button--secondary"
          disabled={isExporting || (!useDefaultLocation && !customDir)}
          onClick={() => void handleExport()}
        >
          {isExporting ? 'Exporting...' : 'Export Diagnostic Bundle'}
        </button>
      </div>

      {result ? (
        <div className="crosshook-settings-help" style={{ marginTop: 8 }}>
          <p>
            <strong>Bundle exported:</strong>{' '}
            <span className="crosshook-muted" title={result.archive_path}>
              {truncatePath(result.archive_path)}
            </span>
          </p>
          <p className="crosshook-muted">
            {result.summary.profile_count} profile{result.summary.profile_count !== 1 ? 's' : ''},{' '}
            {result.summary.log_file_count} log file{result.summary.log_file_count !== 1 ? 's' : ''},{' '}
            {result.summary.proton_install_count} Proton version
            {result.summary.proton_install_count !== 1 ? 's' : ''}
          </p>
        </div>
      ) : null}

      {error ? (
        <p className="crosshook-danger crosshook-settings-error" style={{ marginTop: 8 }}>
          {error}
        </p>
      ) : null}
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

          <DiagnosticExportSection />
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
