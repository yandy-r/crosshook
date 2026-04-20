import { type MouseEvent as ReactMouseEvent, useCallback, useEffect, useId, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { useFocusTrap } from '../hooks/useFocusTrap';
import type { ConfigDiffResult, ConfigRevisionSummary } from '../types/profile-history';
import { formatExactDate } from './config-history/helpers';
import { RestoreConfirmation } from './config-history/RestoreConfirmation';
import { RevisionDetail } from './config-history/RevisionDetail';
import { RevisionTimeline } from './config-history/RevisionTimeline';
import type { ConfigHistoryPanelProps } from './config-history/types';

export type { ConfigHistoryPanelProps };

export function ConfigHistoryPanel({
  profileName,
  onClose,
  fetchConfigHistory,
  fetchConfigDiff,
  rollbackConfig,
  markKnownGood,
  onAfterRollback,
}: ConfigHistoryPanelProps) {
  const portalHostRef = useRef<HTMLElement | null>(null);
  const surfaceRef = useRef<HTMLDivElement | null>(null);
  const headingRef = useRef<HTMLHeadingElement | null>(null);
  const titleId = useId();
  const [isMounted, setIsMounted] = useState(false);

  // Timeline
  const [revisions, setRevisions] = useState<ConfigRevisionSummary[]>([]);
  const [revisionsLoading, setRevisionsLoading] = useState(true);
  const [revisionsError, setRevisionsError] = useState<string | null>(null);

  // Selected revision + diff
  const [selectedRevision, setSelectedRevision] = useState<ConfigRevisionSummary | null>(null);
  const [diff, setDiff] = useState<ConfigDiffResult | null>(null);
  const [diffLoading, setDiffLoading] = useState(false);
  const [diffError, setDiffError] = useState<string | null>(null);

  // Restore confirmation
  const [pendingRestore, setPendingRestore] = useState<ConfigRevisionSummary | null>(null);
  const [restoring, setRestoring] = useState(false);
  const [restoreError, setRestoreError] = useState<string | null>(null);
  const [restoreSuccess, setRestoreSuccess] = useState<string | null>(null);
  /** Best-effort history list refresh failed after a successful restore (disk state is still restored). */
  const [historyRefreshWarning, setHistoryRefreshWarning] = useState<string | null>(null);

  // Mark known good
  const [markingKnownGoodId, setMarkingKnownGoodId] = useState<number | null>(null);
  const [markKnownGoodError, setMarkKnownGoodError] = useState<string | null>(null);

  /* ── Portal setup ── */

  useEffect(() => {
    const host = document.createElement('div');
    host.className = 'crosshook-modal-portal';
    portalHostRef.current = host;
    document.body.appendChild(host);
    setIsMounted(true);

    return () => {
      host.remove();
      portalHostRef.current = null;
      setIsMounted(false);
    };
  }, []);

  /* ── Focus trap ── */

  const { handleKeyDown } = useFocusTrap({
    open: isMounted,
    panelRef: surfaceRef,
    onClose: () => {
      if (pendingRestore) {
        setPendingRestore(null);
        setRestoreError(null);
        return;
      }
      onClose();
    },
    initialFocusRef: headingRef,
  });

  /* ── Load revisions on mount ── */

  const loadRevisions = useCallback(() => {
    let active = true;
    setRevisionsLoading(true);
    setRevisionsError(null);

    fetchConfigHistory(profileName)
      .then((data) => {
        if (!active) return;
        setRevisions(data);
        setRevisionsLoading(false);
      })
      .catch((err: unknown) => {
        if (!active) return;
        setRevisionsError(err instanceof Error ? err.message : String(err));
        setRevisionsLoading(false);
      });

    return () => {
      active = false;
    };
  }, [profileName, fetchConfigHistory]);

  useEffect(() => {
    if (!isMounted) return;
    return loadRevisions();
  }, [isMounted, loadRevisions]);

  /* ── Load diff when revision is selected ── */

  useEffect(() => {
    if (!selectedRevision) {
      setDiff(null);
      setDiffError(null);
      return;
    }

    let active = true;
    setDiffLoading(true);
    setDiffError(null);
    setDiff(null);

    // When the selected revision is the most recent one, compare it against
    // the previous revision so users see what changed in that save. Comparing
    // the latest snapshot against current is always empty (it was just captured).
    const isLatest = revisions.length > 0 && selectedRevision.id === revisions[0].id;
    const previousRevision = isLatest && revisions.length > 1 ? revisions[1] : undefined;
    const rightRevisionId = isLatest ? previousRevision?.id : undefined;

    fetchConfigDiff(profileName, selectedRevision.id, rightRevisionId)
      .then((result) => {
        if (!active) return;
        setDiff(result);
        setDiffLoading(false);
      })
      .catch((err: unknown) => {
        if (!active) return;
        setDiffError(err instanceof Error ? err.message : String(err));
        setDiffLoading(false);
      });

    return () => {
      active = false;
    };
  }, [selectedRevision, revisions, profileName, fetchConfigDiff]);

  /* ── Restore ── */

  const handleConfirmRestore = useCallback(async () => {
    if (!pendingRestore) return;
    setRestoring(true);
    setRestoreError(null);
    setHistoryRefreshWarning(null);
    const snapshotMeta = pendingRestore;
    try {
      await rollbackConfig(profileName, snapshotMeta.id);
      const restoredAt = formatExactDate(snapshotMeta.created_at);
      setRestoreSuccess(`Snapshot restored (${restoredAt}).`);
      setPendingRestore(null);
      setSelectedRevision(null);
      setDiff(null);
      onAfterRollback?.(profileName);
      try {
        const data = await fetchConfigHistory(profileName);
        setRevisions(data);
      } catch (refreshErr: unknown) {
        const detail = refreshErr instanceof Error ? refreshErr.message : String(refreshErr);
        console.warn('Config history list refresh failed after restore:', refreshErr);
        setHistoryRefreshWarning(`Restore completed, but the history list could not be refreshed: ${detail}`);
      }
    } catch (err: unknown) {
      setRestoreError(
        err instanceof Error ? err.message : typeof err === 'string' ? err : 'Restore failed. No changes were applied.'
      );
    } finally {
      setRestoring(false);
    }
  }, [pendingRestore, profileName, rollbackConfig, fetchConfigHistory, onAfterRollback]);

  /* ── Mark known good ── */

  const handleMarkKnownGood = useCallback(
    async (revision: ConfigRevisionSummary) => {
      setMarkingKnownGoodId(revision.id);
      setMarkKnownGoodError(null);
      try {
        await markKnownGood(profileName, revision.id);
        const data = await fetchConfigHistory(profileName);
        setRevisions(data);
        // Update selected revision to reflect new known-good state
        if (selectedRevision?.id === revision.id) {
          const updated = data.find((r) => r.id === revision.id);
          if (updated) setSelectedRevision(updated);
        }
      } catch (err: unknown) {
        const detail = err instanceof Error ? err.message : typeof err === 'string' ? err : String(err);
        console.error('Failed to mark revision as known good:', err);
        setMarkKnownGoodError(`Could not mark this snapshot as known good: ${detail}`);
      } finally {
        setMarkingKnownGoodId(null);
      }
    },
    [profileName, markKnownGood, fetchConfigHistory, selectedRevision]
  );

  /* ── Backdrop click handler ── */

  function handleBackdropMouseDown(event: ReactMouseEvent<HTMLDivElement>) {
    if (event.target !== event.currentTarget) return;
    if (pendingRestore) {
      setPendingRestore(null);
      setRestoreError(null);
      return;
    }
    onClose();
  }

  /* ── Select a revision ── */

  function selectRevision(rev: ConfigRevisionSummary) {
    setSelectedRevision(rev);
    setPendingRestore(null);
    setRestoreError(null);
    setRestoreSuccess(null);
    setHistoryRefreshWarning(null);
    setMarkKnownGoodError(null);
  }

  if (!isMounted || !portalHostRef.current) return null;

  return createPortal(
    <div className="crosshook-modal" role="presentation">
      <div className="crosshook-modal__backdrop" aria-hidden="true" onMouseDown={handleBackdropMouseDown} />
      <div
        ref={surfaceRef}
        className="crosshook-modal__surface crosshook-panel crosshook-focus-scope crosshook-history-panel"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        data-crosshook-focus-root="modal"
        onKeyDown={handleKeyDown}
      >
        {/* Hidden live regions for screen readers */}
        <div
          role="status"
          aria-atomic="true"
          aria-live="polite"
          style={{ position: 'absolute', width: 1, height: 1, overflow: 'hidden', clip: 'rect(0,0,0,0)' }}
        >
          {restoreSuccess ?? ''}
        </div>
        <div
          role="alert"
          aria-atomic="true"
          aria-live="assertive"
          style={{ position: 'absolute', width: 1, height: 1, overflow: 'hidden', clip: 'rect(0,0,0,0)' }}
        >
          {restoreError ?? ''}
        </div>

        {/* Header */}
        <header className="crosshook-modal__header">
          <div className="crosshook-modal__heading-block">
            <div className="crosshook-heading-eyebrow">Configuration history</div>
            <h2 ref={headingRef} id={titleId} className="crosshook-modal__title" tabIndex={-1}>
              {profileName}
            </h2>
          </div>
        </header>

        {/* Body — two-column layout */}
        <div className="crosshook-modal__body crosshook-history-body">
          {/* Timeline column */}
          <RevisionTimeline
            revisions={revisions}
            selectedRevision={selectedRevision}
            profileName={profileName}
            loading={revisionsLoading}
            error={revisionsError}
            onSelectRevision={selectRevision}
          />

          {/* Detail column */}
          <div className="crosshook-history-detail">
            {restoreSuccess ? (
              <div className="crosshook-history-restore-success" role="status" aria-live="polite">
                {restoreSuccess}
              </div>
            ) : null}

            {historyRefreshWarning ? (
              <p
                className="crosshook-help-text"
                role="status"
                aria-live="polite"
                style={{ marginTop: 8, color: 'var(--crosshook-color-text-muted, #9bb1c8)' }}
              >
                {historyRefreshWarning}
              </p>
            ) : null}

            {!selectedRevision ? (
              <div className="crosshook-history-empty" style={{ alignItems: 'flex-start' }}>
                <p className="crosshook-help-text">
                  Select a snapshot from the list to compare it with the current profile.
                </p>
              </div>
            ) : pendingRestore ? (
              /* Restore confirmation */
              <RestoreConfirmation
                revision={pendingRestore}
                restoring={restoring}
                error={restoreError}
                onConfirm={() => void handleConfirmRestore()}
                onCancel={() => {
                  setPendingRestore(null);
                  setRestoreError(null);
                }}
              />
            ) : (
              /* Diff view + actions */
              <RevisionDetail
                revision={selectedRevision}
                diff={diff}
                diffLoading={diffLoading}
                diffError={diffError}
                markingKnownGood={markingKnownGoodId === selectedRevision.id}
                markKnownGoodError={markKnownGoodError}
                onRestore={() => {
                  setRestoreSuccess(null);
                  setRestoreError(null);
                  setMarkKnownGoodError(null);
                  setPendingRestore(selectedRevision);
                }}
                onMarkKnownGood={() => void handleMarkKnownGood(selectedRevision)}
              />
            )}
          </div>
        </div>

        {/* Footer */}
        <footer className="crosshook-modal__footer">
          <span className="crosshook-modal__footer-copy">
            {revisions.length > 0 ? `${revisions.length} snapshot${revisions.length !== 1 ? 's' : ''}` : ''}
          </span>
          <div className="crosshook-modal__footer-actions">
            <button type="button" className="crosshook-button crosshook-button--ghost" onClick={onClose}>
              Close
            </button>
          </div>
        </footer>
      </div>
    </div>,
    portalHostRef.current
  );
}

export default ConfigHistoryPanel;
