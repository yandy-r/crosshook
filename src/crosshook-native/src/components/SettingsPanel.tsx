import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open as openShell } from '@tauri-apps/plugin-shell';
import { useLauncherManagement } from '../hooks/useLauncherManagement';
import { chooseDirectory } from '../utils/dialog';
import { SettingsArt } from './layout/PageBanner';
import { PanelRouteDecor } from './layout/PanelRouteDecor';
import { CollapsibleSection } from './ui/CollapsibleSection';
import type { AppSettingsData, DiagnosticBundleResult } from '../types';

interface RecentFilesState {
  gamePaths: string[];
  trainerPaths: string[];
  dllPaths: string[];
}

export interface SettingsPanelProps {
  settings: AppSettingsData;
  onPersistSettings: (patch: Partial<AppSettingsData>) => Promise<void>;
  recentFiles: RecentFilesState;
  targetHomePath: string;
  steamClientInstallPath: string;
  onAutoLoadLastProfileChange: (enabled: boolean) => void;
  onRefreshRecentFiles?: () => void;
  onClearRecentFiles?: () => void;
  onSteamGridDbApiKeyChange?: (key: string) => Promise<void>;
  onBrowseProfilesDirectory?: () => void;
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
      {error ? <p className="crosshook-danger crosshook-settings-error">{error}</p> : null}

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
        Export a diagnostic bundle containing system info, profiles, logs, and Steam diagnostics as a single .tar.gz
        archive. Attach this to GitHub issues for faster troubleshooting.
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
          <p className="crosshook-muted crosshook-settings-note">Save the bundle to the system temp directory.</p>
        </span>
      </label>

      {!useDefaultLocation ? (
        <div className="crosshook-settings-field-row">
          <label className="crosshook-label">Export directory</label>
          <div className="crosshook-settings-input-row">
            <input className="crosshook-input" value={customDir ?? ''} readOnly placeholder="No directory selected" />
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

function SteamGridDbSection({
  hasApiKey,
  onApiKeyChange,
}: {
  hasApiKey: boolean;
  onApiKeyChange?: (key: string) => Promise<void>;
}) {
  const [localKey, setLocalKey] = useState('');
  const [isSaving, setIsSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const [lastAction, setLastAction] = useState<'save' | 'clear' | null>(null);

  async function handleSave() {
    if (!onApiKeyChange) {
      return;
    }
    setIsSaving(true);
    setSaveError(null);
    setSaved(false);
    try {
      await onApiKeyChange(localKey);
      setLocalKey('');
      setSaved(true);
      setLastAction('save');
    } catch (error) {
      setSaveError(error instanceof Error ? error.message : String(error));
    } finally {
      setIsSaving(false);
    }
  }

  async function handleClear() {
    if (!onApiKeyChange) {
      return;
    }
    setIsSaving(true);
    setSaveError(null);
    setSaved(false);
    try {
      await onApiKeyChange('');
      setLocalKey('');
      setSaved(true);
      setLastAction('clear');
    } catch (error) {
      setSaveError(error instanceof Error ? error.message : String(error));
    } finally {
      setIsSaving(false);
    }
  }

  return (
    <CollapsibleSection
      title="SteamGridDB"
      defaultOpen={false}
      className="crosshook-panel crosshook-settings-section"
      meta={<span className="crosshook-muted">Optional — higher-quality cover art</span>}
    >
      <p className="crosshook-muted crosshook-settings-help">
        Enter your SteamGridDB API key to fetch higher-quality cover art for your game profiles. When set, CrossHook
        will try SteamGridDB before falling back to Steam CDN images.
      </p>

      <div className="crosshook-settings-field-row">
        <span className="crosshook-label">Key status</span>
        {hasApiKey ? (
          <span className="crosshook-muted crosshook-settings-note" style={{ color: 'var(--crosshook-success, #4caf50)' }}>
            Key is set
          </span>
        ) : (
          <span className="crosshook-muted crosshook-settings-note">No key configured</span>
        )}
      </div>

      <div className="crosshook-settings-field-row">
        <label className="crosshook-label" htmlFor="steamgriddb-api-key">
          {hasApiKey ? 'Replace API Key' : 'API Key'}
        </label>
        <div className="crosshook-settings-input-row">
          <input
            id="steamgriddb-api-key"
            type="password"
            className="crosshook-input"
            value={localKey}
            onChange={(event) => {
              setLocalKey(event.target.value);
              setSaved(false);
            }}
            placeholder={hasApiKey ? 'Enter new key to replace the existing one' : 'Enter your SteamGridDB API key'}
            autoComplete="new-password"
          />
          {onApiKeyChange ? (
            <button
              type="button"
              className="crosshook-button crosshook-button--secondary"
              disabled={isSaving || localKey.trim().length === 0}
              onClick={() => void handleSave()}
            >
              {isSaving ? 'Saving...' : 'Save'}
            </button>
          ) : null}
        </div>
      </div>

      {hasApiKey && onApiKeyChange ? (
        <div className="crosshook-settings-clear-row">
          <button
            type="button"
            className="crosshook-button crosshook-button--ghost"
            disabled={isSaving}
            onClick={() => void handleClear()}
          >
            Clear API key
          </button>
        </div>
      ) : null}

      {saved ? (
        <p className="crosshook-muted crosshook-settings-note" style={{ color: 'var(--crosshook-success, #4caf50)' }}>
          {lastAction === 'clear' ? 'API key cleared.' : 'API key saved.'}
        </p>
      ) : null}

      {saveError ? (
        <p className="crosshook-danger crosshook-settings-error" style={{ marginTop: 4 }}>
          {saveError}
        </p>
      ) : null}

      <p className="crosshook-muted crosshook-settings-note">
        The key is stored in <code>~/.config/crosshook/settings.toml</code>. Avoid syncing this file to public
        repositories.
      </p>

      <div className="crosshook-settings-clear-row">
        <button
          type="button"
          className="crosshook-button crosshook-button--outline"
          onClick={() => void openShell('https://www.steamgriddb.com/')}
        >
          Get API Key at steamgriddb.com ↗
        </button>
      </div>
    </CollapsibleSection>
  );
}

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

  const profilesDirectoryMessage = settings.profiles_directory.trim()
    ? 'Custom path is saved in settings.toml. Restart CrossHook to use it as the active profile store.'
    : 'Leave empty to use the default directory under your CrossHook config folder.';

  return (
    <section className="crosshook-card crosshook-card--with-route-decor crosshook-settings-panel" aria-label="Settings">
      <PanelRouteDecor illustration={<SettingsArt />} />
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
          <span>{settings.last_used_profile.trim().length > 0 ? settings.last_used_profile : 'none'}</span>
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
                checked={settings.auto_load_last_profile}
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
                value={settings.last_used_profile}
                readOnly
                placeholder="No profile selected"
              />
            </div>
          </CollapsibleSection>

          <CollapsibleSection
            title="New profile defaults"
            className="crosshook-panel crosshook-settings-section"
            meta={<span className="crosshook-muted">settings.toml</span>}
          >
            <p className="crosshook-muted crosshook-settings-help">
              Applied when you save a profile for the first time (new name). Empty fields keep CrossHook&apos;s built-in
              detection.
            </p>
            <div className="crosshook-settings-field-row">
              <label className="crosshook-label" htmlFor="default-proton-path">
                Default Proton path
              </label>
              <input
                id="default-proton-path"
                key={`dp-${settings.default_proton_path}`}
                className="crosshook-input"
                defaultValue={settings.default_proton_path}
                placeholder="/path/to/proton"
                onBlur={(event) => {
                  const v = event.target.value.trim();
                  if (v !== settings.default_proton_path.trim()) {
                    void onPersistSettings({ default_proton_path: v });
                  }
                }}
              />
            </div>
            <div className="crosshook-settings-field-row">
              <label className="crosshook-label" htmlFor="default-launch-method">
                Default launch method
              </label>
              <select
                id="default-launch-method"
                className="crosshook-input"
                value={settings.default_launch_method}
                onChange={(event) => void onPersistSettings({ default_launch_method: event.target.value })}
              >
                <option value="">Auto (from game / Steam)</option>
                <option value="proton_run">proton_run</option>
                <option value="steam_applaunch">steam_applaunch</option>
                <option value="native">native</option>
              </select>
            </div>
            <div className="crosshook-settings-field-row">
              <label className="crosshook-label" htmlFor="default-trainer-mode">
                Default trainer loading mode
              </label>
              <select
                id="default-trainer-mode"
                className="crosshook-input"
                value={settings.default_trainer_loading_mode}
                onChange={(event) =>
                  void onPersistSettings({ default_trainer_loading_mode: event.target.value })
                }
              >
                <option value="source_directory">source_directory</option>
                <option value="copy_to_prefix">copy_to_prefix</option>
              </select>
            </div>
            <div className="crosshook-settings-field-row">
              <label className="crosshook-label" htmlFor="default-bundled-preset">
                Default bundled optimization preset id
              </label>
              <input
                id="default-bundled-preset"
                key={`dbp-${settings.default_bundled_optimization_preset_id}`}
                className="crosshook-input"
                defaultValue={settings.default_bundled_optimization_preset_id}
                placeholder="e.g. preset id from metadata catalog"
                onBlur={(event) => {
                  const v = event.target.value.trim();
                  if (v !== settings.default_bundled_optimization_preset_id.trim()) {
                    void onPersistSettings({ default_bundled_optimization_preset_id: v });
                  }
                }}
              />
            </div>
          </CollapsibleSection>

          <CollapsibleSection
            title="Logging and UI"
            className="crosshook-panel crosshook-settings-section"
            meta={<span className="crosshook-muted">settings.toml</span>}
          >
            <div className="crosshook-settings-field-row">
              <label className="crosshook-label" htmlFor="log-filter">
                Backend log filter
              </label>
              <input
                id="log-filter"
                key={`lf-${settings.log_filter}`}
                className="crosshook-input"
                defaultValue={settings.log_filter}
                placeholder="info"
                onBlur={(event) => {
                  const v = event.target.value.trim() || 'info';
                  if (v !== settings.log_filter.trim()) {
                    void onPersistSettings({ log_filter: v });
                  }
                }}
              />
            </div>
            <p className="crosshook-muted crosshook-settings-note">
              If <code>RUST_LOG</code> is set in the environment, it overrides this value. Otherwise this filter applies
              at startup (for example <code>info</code>, <code>debug</code>, or <code>crosshook_core=debug</code>).
              Restart the app after changing.
            </p>
            <label className="crosshook-settings-checkbox-row">
              <input
                type="checkbox"
                checked={!settings.console_drawer_collapsed_default}
                onChange={(event) =>
                  void onPersistSettings({ console_drawer_collapsed_default: !event.target.checked })
                }
                className="crosshook-settings-checkbox"
              />
              <span>
                <span className="crosshook-label">Start with console drawer expanded</span>
                <p className="crosshook-muted crosshook-settings-note">
                  When off, the drawer starts collapsed until launch logs arrive.
                </p>
              </span>
            </label>
            <div className="crosshook-settings-field-row">
              <label className="crosshook-label" htmlFor="recent-files-limit">
                Recent files limit (per list)
              </label>
              <input
                id="recent-files-limit"
                type="number"
                min={1}
                max={100}
                className="crosshook-input"
                style={{ maxWidth: 120 }}
                defaultValue={settings.recent_files_limit}
                key={`rfl-${settings.recent_files_limit}`}
                onBlur={(event) => {
                  const raw = parseInt(event.target.value, 10);
                  if (!Number.isFinite(raw)) return;
                  const v = Math.min(100, Math.max(1, raw));
                  if (v !== settings.recent_files_limit) {
                    void onPersistSettings({ recent_files_limit: v });
                  }
                }}
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

          <ManageLaunchersSection targetHomePath={targetHomePath} steamClientInstallPath={steamClientInstallPath} />

          <DiagnosticExportSection />

          <SteamGridDbSection
            hasApiKey={settings.has_steamgriddb_api_key}
            onApiKeyChange={onSteamGridDbApiKeyChange}
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
