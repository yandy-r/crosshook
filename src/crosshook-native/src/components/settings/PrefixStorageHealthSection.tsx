import { useEffect, useMemo, useState } from 'react';
import type { ScanSource } from '../../hooks/usePrefixStorageManagement';
import { usePrefixStorageManagement } from '../../hooks/usePrefixStorageManagement';
import type {
  PrefixCleanupResult,
  PrefixCleanupTargetKind,
  PrefixStorageCleanupAuditRow,
  PrefixStorageEntry,
  StaleStagedTrainerEntry,
} from '../../types/prefix-storage';
import { CollapsibleSection } from '../ui/CollapsibleSection';
import { formatBytes, formatTimestamp, truncatePath } from './format';

// ---------------------------------------------------------------------------
// Private sub-components
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Public section component
// ---------------------------------------------------------------------------

/** Collapsible section for scanning and cleaning up CrossHook-managed Wine prefixes. */
export function PrefixStorageHealthSection() {
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
