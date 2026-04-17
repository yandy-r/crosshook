import { useEffect, useMemo, useState } from 'react';
import { callCommand } from '@/lib/ipc';
import { open as openShell } from '@/lib/plugin-stubs/shell';
import { useLauncherManagement } from '../hooks/useLauncherManagement';
import type { ScanSource } from '../hooks/usePrefixStorageManagement';
import { usePrefixStorageManagement } from '../hooks/usePrefixStorageManagement';
import { useUmuDatabaseRefresh } from '../hooks/useUmuDatabaseRefresh';
import type { AppSettingsData, DiagnosticBundleResult, UmuPreference } from '../types';
import type {
  PrefixCleanupResult,
  PrefixCleanupTargetKind,
  PrefixStorageCleanupAuditRow,
  PrefixStorageEntry,
  StaleStagedTrainerEntry,
} from '../types/prefix-storage';
import type { InstallRootDescriptor, ProtonUpProviderDescriptor } from '../types/protonup';
import { chooseDirectory, chooseFile } from '../utils/dialog';
import { CollapsibleSection } from './ui/CollapsibleSection';
import { ThemedSelect } from './ui/ThemedSelect';

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

function formatBytes(value: number) {
  if (!Number.isFinite(value) || value <= 0) {
    return '0 B';
  }
  const units = ['B', 'KiB', 'MiB', 'GiB', 'TiB'];
  let current = value;
  let unitIndex = 0;
  while (current >= 1024 && unitIndex < units.length - 1) {
    current /= 1024;
    unitIndex += 1;
  }
  return `${current.toFixed(current >= 100 ? 0 : current >= 10 ? 1 : 2)} ${units[unitIndex]}`;
}

function formatTimestamp(value: string | null) {
  if (!value) {
    return 'Unknown';
  }
  const parsed = new Date(value);
  if (!Number.isFinite(parsed.getTime())) {
    return value;
  }
  return parsed.toLocaleString();
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

function PrefixStorageSummary({
  scannedAt,
  prefixCount,
  orphanCount,
  staleCount,
  scanSource,
}: {
  scannedAt: string;
  prefixCount: number;
  orphanCount: number;
  staleCount: number;
  scanSource?: ScanSource;
}) {
  return (
    <div className="crosshook-settings-storage-summary">
      <span className="crosshook-status-chip">
        <strong>Last scan:</strong>
        <span>{formatTimestamp(scannedAt)}</span>
        {scanSource ? (
          <span
            className={`crosshook-health-chip crosshook-health-chip--${scanSource === 'live' ? 'healthy' : 'warning'}`}
          >
            {scanSource === 'live' ? 'Live' : 'Cached'}
          </span>
        ) : null}
      </span>
      <span className="crosshook-status-chip">
        <strong>Prefixes:</strong>
        <span>{prefixCount}</span>
      </span>
      <span className="crosshook-status-chip">
        <strong>Orphans:</strong>
        <span>{orphanCount}</span>
      </span>
      <span className="crosshook-status-chip">
        <strong>Stale staged:</strong>
        <span>{staleCount}</span>
      </span>
    </div>
  );
}

function PrefixCleanupActionRow({
  kind,
  confirmAction,
  count,
  cleanupLoading,
  onSetConfirmAction,
  onConfirmCleanup,
}: {
  kind: PrefixCleanupTargetKind;
  confirmAction: PrefixCleanupTargetKind | null;
  count: number;
  cleanupLoading: boolean;
  onSetConfirmAction: (value: PrefixCleanupTargetKind | null) => void;
  onConfirmCleanup: (kind: PrefixCleanupTargetKind) => void;
}) {
  const isConfirming = confirmAction === kind;
  const isDisabled = cleanupLoading || count === 0;
  const label = kind === 'orphan_prefix' ? 'Orphan Prefixes' : 'Stale Staged Entries';
  const confirmLabel = kind === 'orphan_prefix' ? 'Orphan Prefixes' : 'Stale Staged Entries';

  return (
    <div className="crosshook-settings-storage-actions">
      {isConfirming ? (
        <>
          <button
            type="button"
            className="crosshook-button crosshook-button--danger crosshook-settings-small-button"
            disabled={isDisabled}
            onClick={() => onConfirmCleanup(kind)}
          >
            {cleanupLoading ? 'Cleaning...' : `Confirm Remove ${count} ${confirmLabel}`}
          </button>
          <button
            type="button"
            className="crosshook-button crosshook-button--ghost crosshook-button--ghost--small crosshook-settings-small-button"
            onClick={() => onSetConfirmAction(null)}
          >
            Cancel
          </button>
        </>
      ) : (
        <button
          type="button"
          className="crosshook-button crosshook-button--warning crosshook-settings-small-button"
          disabled={isDisabled}
          onClick={() => onSetConfirmAction(kind)}
        >
          Remove {label} ({count})
        </button>
      )}
    </div>
  );
}

function PrefixCleanupOutcome({ cleanupResult }: { cleanupResult: PrefixCleanupResult }) {
  return (
    <div className="crosshook-settings-storage-outcome">
      <p className="crosshook-muted crosshook-settings-note">
        Cleanup result: deleted {cleanupResult.deleted.length}, skipped {cleanupResult.skipped.length}, reclaimed{' '}
        {formatBytes(cleanupResult.reclaimed_bytes)}.
      </p>
      {cleanupResult.skipped.length > 0 ? (
        <ul className="crosshook-recent-list">
          {cleanupResult.skipped.slice(0, 3).map((skip) => (
            <li key={`${skip.target.kind}-${skip.target.target_path}`} className="crosshook-recent-item">
              <div className="crosshook-recent-item__label">{truncatePath(skip.target.target_path)}</div>
              <div className="crosshook-muted crosshook-settings-note">{skip.reason}</div>
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  );
}

function PrefixStorageTopStaleList({ entries }: { entries: StaleStagedTrainerEntry[] }) {
  if (entries.length === 0) {
    return null;
  }

  return (
    <section className="crosshook-settings-storage-targets">
      <div className="crosshook-settings-section-header">
        <div className="crosshook-heading-eyebrow">Largest stale staged entries</div>
        <div className="crosshook-muted crosshook-settings-meta">Top {entries.length} by size</div>
      </div>
      <ul className="crosshook-recent-list">
        {entries.map((entry) => (
          <li key={entry.target_path} className="crosshook-recent-item">
            <div className="crosshook-recent-item__label">{truncatePath(entry.target_path)}</div>
            <div className="crosshook-muted crosshook-settings-note">
              {entry.entry_name} | {formatBytes(entry.total_bytes)} | Modified {formatTimestamp(entry.modified_at)}
            </div>
          </li>
        ))}
      </ul>
    </section>
  );
}

function PrefixStorageList({ entries }: { entries: PrefixStorageEntry[] }) {
  return (
    <ul className="crosshook-recent-list">
      {entries.map((entry) => (
        <li key={entry.resolved_prefix_path} className="crosshook-recent-item">
          <div className="crosshook-settings-launcher-row">
            <div>
              <div className="crosshook-recent-item__label crosshook-settings-launcher-label">
                {truncatePath(entry.resolved_prefix_path)}
                {entry.is_orphan ? (
                  <span className="crosshook-health-chip crosshook-health-chip--warning" style={{ marginLeft: 8 }}>
                    Orphan
                  </span>
                ) : null}
              </div>
              <div className="crosshook-recent-item__label crosshook-settings-launcher-path">
                Total {formatBytes(entry.total_bytes)} | Staged {formatBytes(entry.staged_trainers_bytes)} | Stale{' '}
                {entry.stale_staged_trainers.length}
              </div>
              <div className="crosshook-muted crosshook-settings-note">
                {entry.referenced_by_profiles.length > 0
                  ? `Profiles: ${entry.referenced_by_profiles.join(', ')}`
                  : 'No profile references'}
              </div>
            </div>
          </div>
        </li>
      ))}
    </ul>
  );
}

function PrefixStorageCleanupHistory({ entries }: { entries: PrefixStorageCleanupAuditRow[] }) {
  const displayEntries = entries.filter((e) => e.target_kind !== 'summary');
  if (displayEntries.length === 0) {
    return <p className="crosshook-muted crosshook-settings-note">No cleanup history recorded yet.</p>;
  }
  return (
    <ul className="crosshook-list crosshook-list--compact">
      {displayEntries.map((entry) => (
        <li key={entry.id} className="crosshook-list-item">
          <div className="crosshook-list-item-content">
            <span
              className={`crosshook-health-chip crosshook-health-chip--${entry.result === 'deleted' ? 'healthy' : 'warning'}`}
            >
              {entry.result}
            </span>
            <span className="crosshook-muted" style={{ fontSize: '0.85em' }}>
              {entry.target_kind === 'orphan_prefix' ? 'Orphan prefix' : 'Stale staged trainer'}
            </span>
            <span className="crosshook-truncate" title={entry.target_path} style={{ maxWidth: '300px' }}>
              {entry.target_path}
            </span>
            {entry.reason ? <span className="crosshook-muted">({entry.reason})</span> : null}
            <span className="crosshook-muted" style={{ marginLeft: 'auto', fontSize: '0.85em' }}>
              {formatTimestamp(entry.created_at)}
            </span>
          </div>
        </li>
      ))}
    </ul>
  );
}

function PrefixStorageHealthSection() {
  const {
    scanResult,
    scanLoading,
    cleanupLoading,
    error,
    scanStorage,
    cleanupStorage,
    scanSource,
    persistenceAvailable,
    auditEntries,
    loadHistory,
  } = usePrefixStorageManagement();
  const [confirmAction, setConfirmAction] = useState<PrefixCleanupTargetKind | null>(null);
  const [cleanupResult, setCleanupResult] = useState<PrefixCleanupResult | null>(null);
  const [cleanupError, setCleanupError] = useState<string | null>(null);

  useEffect(() => {
    void loadHistory();
  }, [loadHistory]);

  const orphanTargets = scanResult?.orphan_targets ?? [];
  const staleTargets = scanResult?.stale_staged_targets ?? [];
  const allPrefixes = scanResult?.prefixes ?? [];
  const displayError = cleanupError ?? error;
  const sortedPrefixes = useMemo(
    () =>
      [...allPrefixes].sort((left, right) => {
        if (left.is_orphan !== right.is_orphan) {
          return left.is_orphan ? -1 : 1;
        }
        return right.total_bytes - left.total_bytes;
      }),
    [allPrefixes]
  );
  const topStaleEntries = useMemo(
    () =>
      allPrefixes
        .flatMap((entry) => entry.stale_staged_trainers)
        .sort((left, right) => right.total_bytes - left.total_bytes)
        .slice(0, 5),
    [allPrefixes]
  );

  async function handleCleanup(kind: PrefixCleanupTargetKind) {
    const targets = kind === 'orphan_prefix' ? orphanTargets : staleTargets;
    if (targets.length === 0) {
      return;
    }
    setCleanupError(null);
    try {
      const result = await cleanupStorage(targets);
      setCleanupResult(result);
      setConfirmAction(null);
      await scanStorage();
    } catch (cleanupRunError) {
      setCleanupError(cleanupRunError instanceof Error ? cleanupRunError.message : String(cleanupRunError));
    }
  }

  return (
    <CollapsibleSection
      title="Prefix Storage Health"
      defaultOpen={false}
      className="crosshook-panel crosshook-settings-section"
      meta={<span className="crosshook-muted">CrossHook-managed prefixes only</span>}
    >
      <p className="crosshook-muted crosshook-settings-help">
        Scans are local-only. Cleanup is always manual, confirmation-gated, and limited to CrossHook-managed prefix
        paths.
      </p>
      {!persistenceAvailable ? (
        <p className="crosshook-muted crosshook-settings-note" style={{ fontStyle: 'italic' }}>
          Scan history is unavailable (metadata database offline). Scanning and cleanup still work normally.
        </p>
      ) : null}
      <div className="crosshook-settings-clear-row">
        <button
          type="button"
          className="crosshook-button crosshook-button--secondary"
          disabled={scanLoading || cleanupLoading}
          onClick={() => {
            setConfirmAction(null);
            setCleanupResult(null);
            setCleanupError(null);
            void scanStorage().catch(() => undefined);
          }}
        >
          {scanLoading ? 'Scanning...' : scanResult ? 'Refresh Scan' : 'Run Scan'}
        </button>
      </div>

      {displayError ? <p className="crosshook-danger crosshook-settings-error">{displayError}</p> : null}

      {scanResult ? (
        <>
          <PrefixStorageSummary
            scannedAt={scanResult.scanned_at}
            prefixCount={allPrefixes.length}
            orphanCount={orphanTargets.length}
            staleCount={staleTargets.length}
            scanSource={scanSource}
          />
          <PrefixCleanupActionRow
            kind="orphan_prefix"
            confirmAction={confirmAction}
            count={orphanTargets.length}
            cleanupLoading={cleanupLoading}
            onSetConfirmAction={setConfirmAction}
            onConfirmCleanup={(kind) => void handleCleanup(kind)}
          />
          <PrefixCleanupActionRow
            kind="stale_staged_trainer"
            confirmAction={confirmAction}
            count={staleTargets.length}
            cleanupLoading={cleanupLoading}
            onSetConfirmAction={setConfirmAction}
            onConfirmCleanup={(kind) => void handleCleanup(kind)}
          />
          {cleanupResult ? <PrefixCleanupOutcome cleanupResult={cleanupResult} /> : null}
          <PrefixStorageTopStaleList entries={topStaleEntries} />
          <PrefixStorageList entries={sortedPrefixes} />
        </>
      ) : (
        <p className="crosshook-muted crosshook-settings-note">Run a scan to view prefix size and cleanup targets.</p>
      )}

      {persistenceAvailable && auditEntries.length > 0 ? (
        <CollapsibleSection title="Cleanup History" defaultOpen={false} className="crosshook-settings-subsection">
          <PrefixStorageCleanupHistory entries={auditEntries} />
        </CollapsibleSection>
      ) : null}
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
      const bundleResult = await callCommand<DiagnosticBundleResult>('export_diagnostics', {
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
          <span className="crosshook-label" id="settings-export-directory-label">
            Export directory
          </span>
          <div className="crosshook-settings-input-row">
            <input
              className="crosshook-input"
              value={customDir ?? ''}
              readOnly
              placeholder="No directory selected"
              aria-labelledby="settings-export-directory-label"
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
          <span
            className="crosshook-muted crosshook-settings-note"
            style={{ color: 'var(--crosshook-success, #4caf50)' }}
          >
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

function ProtonManagerDefaultsSection({
  settings,
  steamClientInstallPath,
  onPersistSettings,
}: {
  settings: AppSettingsData;
  steamClientInstallPath: string;
  onPersistSettings: (patch: Partial<AppSettingsData>) => Promise<void>;
}) {
  const [providers, setProviders] = useState<ProtonUpProviderDescriptor[]>([]);
  const [roots, setRoots] = useState<InstallRootDescriptor[]>([]);

  useEffect(() => {
    let active = true;

    void callCommand<ProtonUpProviderDescriptor[]>('protonup_list_providers')
      .then((result) => {
        if (active) setProviders(result);
      })
      .catch(() => {
        if (active) setProviders([]);
      });

    void callCommand<InstallRootDescriptor[]>('protonup_resolve_install_roots', {
      steam_client_install_path: steamClientInstallPath.length > 0 ? steamClientInstallPath : undefined,
    })
      .then((result) => {
        if (active) setRoots(result);
      })
      .catch(() => {
        if (active) setRoots([]);
      });

    return () => {
      active = false;
    };
  }, [steamClientInstallPath]);

  const installableProviders = useMemo(() => providers.filter((p) => p.supports_install), [providers]);

  return (
    <CollapsibleSection
      title="Proton manager defaults"
      defaultOpen={false}
      className="crosshook-panel crosshook-settings-section"
      meta={<span className="crosshook-muted">Native Proton download manager</span>}
    >
      <p className="crosshook-muted crosshook-settings-help">
        These settings control the native Proton download manager. They do not affect the legacy ProtonUp-Qt advisory
        suggestions.
      </p>

      {installableProviders.length > 0 ? (
        <div className="crosshook-settings-field-row">
          <label className="crosshook-label" htmlFor="protonup-default-provider">
            Default provider
          </label>
          <select
            id="protonup-default-provider"
            className="crosshook-input"
            value={settings.protonup_default_provider ?? ''}
            onChange={(event) => void onPersistSettings({ protonup_default_provider: event.target.value })}
          >
            <option value="">Auto (first available)</option>
            {installableProviders.map((p) => (
              <option key={p.id} value={p.id}>
                {p.display_name}
              </option>
            ))}
          </select>
        </div>
      ) : null}

      {roots.length > 0 ? (
        <div className="crosshook-settings-field-row">
          <label className="crosshook-label" htmlFor="protonup-default-install-root">
            Default install root
          </label>
          <select
            id="protonup-default-install-root"
            className="crosshook-input"
            value={settings.protonup_default_install_root ?? ''}
            onChange={(event) => void onPersistSettings({ protonup_default_install_root: event.target.value })}
          >
            <option value="">Auto-pick (first writable)</option>
            {roots.map((r) => (
              <option key={r.path} value={r.path} disabled={!r.writable}>
                {r.path}
                {r.writable ? '' : ' (read-only)'}
              </option>
            ))}
          </select>
        </div>
      ) : null}

      <label className="crosshook-settings-checkbox-row">
        <input
          type="checkbox"
          checked={settings.protonup_include_prereleases ?? false}
          onChange={(event) => void onPersistSettings({ protonup_include_prereleases: event.target.checked })}
          className="crosshook-settings-checkbox"
        />
        <span>
          <span className="crosshook-label">Include pre-release versions</span>
          <p className="crosshook-muted crosshook-settings-note">
            When enabled, the native Proton manager catalog will include release candidates and beta versions.
          </p>
        </span>
      </label>
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

  const [binaryDetection, setBinaryDetection] = useState<{
    found: boolean;
    binary_name: string;
    source: string;
  } | null>(null);

  const { isRefreshing, lastRefreshStatus, refresh: onRefreshUmuDatabase } = useUmuDatabaseRefresh();

  useEffect(() => {
    let active = true;
    try {
      void callCommand<{ found: boolean; binary_name: string; source: string }>('detect_protontricks_binary')
        .then((result) => {
          if (active) setBinaryDetection(result);
        })
        .catch(() => {
          if (active) setBinaryDetection(null);
        });
    } catch {
      if (active) setBinaryDetection(null);
    }
    return () => {
      active = false;
    };
  }, []);

  return (
    <section className="crosshook-card crosshook-settings-panel" aria-label="Settings">
      <header className="crosshook-settings-header">
        <h2 className="crosshook-heading-title crosshook-heading-title--card">App preferences and storage</h2>
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
            defaultOpen={false}
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
                onChange={(event) => void onPersistSettings({ default_trainer_loading_mode: event.target.value })}
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
            title="Runner"
            defaultOpen={false}
            className="crosshook-panel crosshook-settings-section"
            meta={<span className="crosshook-muted">settings.toml</span>}
          >
            <p className="crosshook-muted crosshook-settings-help">
              Global runner applied to every launch. Individual profiles can override this in their Runtime section.
            </p>
            <div className="crosshook-settings-field-row">
              <label className="crosshook-label" htmlFor="umu-preference" id="umu-preference-label">
                Runner (global default)
              </label>
              <ThemedSelect
                id="umu-preference"
                ariaLabelledby="umu-preference-label"
                value={settings.umu_preference}
                onValueChange={(value) => void onPersistSettings({ umu_preference: value as UmuPreference })}
                options={[
                  { value: 'auto', label: 'Auto (umu when available, else Proton)' },
                  { value: 'umu', label: 'Umu (umu-launcher)' },
                  { value: 'proton', label: 'Proton (direct)' },
                ]}
              />
            </div>
            <div className="crosshook-settings-field-row">
              <span className="crosshook-label">umu protonfix database</span>
              <div>
                <button
                  type="button"
                  className="crosshook-button"
                  onClick={() => void onRefreshUmuDatabase()}
                  disabled={isRefreshing}
                >
                  {isRefreshing ? 'Refreshing…' : 'Refresh umu protonfix database'}
                </button>
                <div className="crosshook-muted" style={{ fontSize: '0.85rem', marginTop: 4 }}>
                  {lastRefreshStatus?.cached_at
                    ? `Last refreshed: ${new Date(lastRefreshStatus.cached_at).toLocaleString()}`
                    : lastRefreshStatus
                      ? `Status: ${lastRefreshStatus.reason}`
                      : 'Not refreshed this session'}
                </div>
              </div>
            </div>
          </CollapsibleSection>

          <ProtonManagerDefaultsSection
            settings={settings}
            steamClientInstallPath={steamClientInstallPath}
            onPersistSettings={onPersistSettings}
          />

          <CollapsibleSection
            title="Logging and UI"
            defaultOpen={false}
            className="crosshook-panel crosshook-settings-section"
            meta={<span className="crosshook-muted">settings.toml</span>}
          >
            <div className="crosshook-settings-field-row">
              <label className="crosshook-label" htmlFor="log-filter">
                Log detail level
              </label>
              <select
                id="log-filter"
                className="crosshook-input"
                value={settings.log_filter}
                onChange={(event) => {
                  const v = event.target.value;
                  if (v !== settings.log_filter) {
                    void onPersistSettings({ log_filter: v });
                  }
                }}
              >
                <option value="error">Error — critical issues only</option>
                <option value="warn">Warning — errors and warnings</option>
                <option value="info">Info — general activity (default)</option>
                <option value="debug">Debug — detailed diagnostics</option>
                <option value="trace">Trace — everything (verbose)</option>
              </select>
            </div>
            <p className="crosshook-muted crosshook-settings-note">
              Controls how much detail appears in the backend logs. Higher levels include more output and may affect
              performance. Restart the app after changing.
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
            title="Prefix Dependencies"
            defaultOpen={false}
            className="crosshook-panel crosshook-settings-section"
            meta={<span className="crosshook-muted">Winetricks / Protontricks</span>}
          >
            <div className="crosshook-settings-field-row">
              <label className="crosshook-label" htmlFor="protontricks-binary-path">
                Winetricks/Protontricks Binary Path
              </label>
              <div className="crosshook-settings-input-row">
                <input
                  id="protontricks-binary-path"
                  key={`ptbp-${settings.protontricks_binary_path}`}
                  className="crosshook-input"
                  defaultValue={settings.protontricks_binary_path}
                  placeholder="/usr/bin/protontricks"
                  onBlur={(event) => {
                    const v = event.target.value.trim();
                    if (v !== settings.protontricks_binary_path.trim()) {
                      void onPersistSettings({ protontricks_binary_path: v });
                    }
                  }}
                />
                <button
                  type="button"
                  className="crosshook-button crosshook-button--secondary"
                  onClick={() => {
                    void (async () => {
                      const path = await chooseFile('Select winetricks or protontricks binary');
                      if (path) void onPersistSettings({ protontricks_binary_path: path });
                    })();
                  }}
                >
                  Browse…
                </button>
              </div>
            </div>
            <p className="crosshook-muted crosshook-settings-note">
              If left empty, CrossHook will auto-detect winetricks/protontricks from PATH.
            </p>
            {binaryDetection ? (
              <p
                className={binaryDetection.found ? 'crosshook-success' : 'crosshook-warning'}
                style={{ fontSize: '0.85rem', margin: '4px 0 0' }}
                aria-live="polite"
              >
                {binaryDetection.found
                  ? `Binary found: ${binaryDetection.binary_name} (source: ${binaryDetection.source})`
                  : 'No winetricks or protontricks binary found'}
              </p>
            ) : null}

            <label className="crosshook-settings-checkbox-row">
              <input
                type="checkbox"
                checked={settings.auto_install_prefix_deps}
                onChange={(event) => void onPersistSettings({ auto_install_prefix_deps: event.target.checked })}
                className="crosshook-settings-checkbox"
              />
              <span>
                <span className="crosshook-label">Auto-install prefix dependencies on first launch</span>
                <p className="crosshook-muted crosshook-settings-note">
                  When enabled, CrossHook will automatically install any required winetricks/protontricks dependencies
                  into the Wine prefix before launching for the first time.
                </p>
              </span>
            </label>
          </CollapsibleSection>

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

          <ManageLaunchersSection targetHomePath={targetHomePath} steamClientInstallPath={steamClientInstallPath} />
          <PrefixStorageHealthSection />

          <DiagnosticExportSection />

          <SteamGridDbSection hasApiKey={settings.has_steamgriddb_api_key} onApiKeyChange={onSteamGridDbApiKeyChange} />
        </div>

        <section className="crosshook-settings-recent-column" aria-label="Recent files">
          <CollapsibleSection
            title="Recent Files"
            defaultOpen={false}
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
