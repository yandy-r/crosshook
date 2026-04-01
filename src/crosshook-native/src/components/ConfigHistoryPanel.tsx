import { createPortal } from 'react-dom';
import {
  useCallback,
  useEffect,
  useId,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent as ReactMouseEvent,
} from 'react';
import type { ConfigDiffResult, ConfigRevisionSource, ConfigRevisionSummary } from '../types/profile-history';
import { formatRelativeTime } from '../utils/format';

/* ── Focus-trap helpers (mirrors ProfilePreviewModal) ── */

const FOCUSABLE_SELECTOR = [
  'a[href]',
  'button:not([disabled])',
  'input:not([disabled]):not([type="hidden"])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
  '[contenteditable="true"]',
].join(', ');

function getFocusableElements(container: HTMLElement): HTMLElement[] {
  return Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(
    (el) => !el.hasAttribute('disabled') && el.tabIndex >= 0 && el.getClientRects().length > 0
  );
}

function focusElement(element: HTMLElement | null): boolean {
  if (!element) return false;
  element.focus({ preventScroll: true });
  return document.activeElement === element;
}

/* ── Source label map ── */

const SOURCE_LABELS: Record<ConfigRevisionSource, string> = {
  manual_save: 'Manual save',
  rollback_apply: 'Restore',
  import: 'Import',
  launch_optimization_save: 'Optimization save',
  preset_apply: 'Preset applied',
  migration: 'Migration',
};

function formatExactDate(isoString: string): string {
  try {
    return new Date(isoString).toLocaleString(undefined, {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    });
  } catch {
    return isoString;
  }
}

/* ── Diff renderer ── */

function DiffView({ diff }: { diff: ConfigDiffResult }) {
  if (diff.diff_text.trim() === '') {
    return (
      <p className="crosshook-help-text" style={{ marginTop: 8 }}>
        No differences found.
      </p>
    );
  }

  const lines = diff.diff_text.split('\n');

  return (
    <div>
      <div className="crosshook-history-diff-stats">
        <span className="crosshook-history-stat--add">+{diff.added_lines} added</span>
        <span className="crosshook-history-stat--remove">-{diff.removed_lines} removed</span>
        {diff.truncated && (
          <span className="crosshook-help-text" style={{ marginLeft: 8 }}>
            (truncated — profile exceeds 2 000 lines)
          </span>
        )}
      </div>
      <pre className="crosshook-history-diff-code" aria-label="Unified diff">
        {lines.map((line, idx) => {
          let cls = 'crosshook-history-diff-line';
          if (line.startsWith('+') && !line.startsWith('+++')) {
            cls += ' crosshook-history-diff-line--add';
          } else if (line.startsWith('-') && !line.startsWith('---')) {
            cls += ' crosshook-history-diff-line--remove';
          } else if (line.startsWith('@@')) {
            cls += ' crosshook-history-diff-line--meta';
          }
          // eslint-disable-next-line react/no-array-index-key
          return (
            <span key={idx} className={cls}>
              {line}
              {'\n'}
            </span>
          );
        })}
      </pre>
    </div>
  );
}

/* ── ConfigHistoryPanel ── */

export interface ConfigHistoryPanelProps {
  profileName: string;
  onClose: () => void;
  fetchConfigHistory: (profileName: string, limit?: number) => Promise<ConfigRevisionSummary[]>;
  fetchConfigDiff: (profileName: string, revisionId: number, rightRevisionId?: number) => Promise<ConfigDiffResult>;
  rollbackConfig: (profileName: string, revisionId: number) => Promise<unknown>;
  markKnownGood: (profileName: string, revisionId: number) => Promise<void>;
  /** Called after a successful rollback so the caller can refresh health data. */
  onAfterRollback?: (profileName: string) => void;
}

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
  const previouslyFocusedRef = useRef<HTMLElement | null>(null);
  const bodyStyleRef = useRef('');
  const hiddenNodesRef = useRef<Array<{ element: HTMLElement; inert: boolean; ariaHidden: string | null }>>([]);
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

  /* ── Focus trap + scroll lock ── */

  useEffect(() => {
    if (!isMounted) return;

    const { body } = document;
    const portalHost = portalHostRef.current;
    if (!portalHost) return;

    previouslyFocusedRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;

    bodyStyleRef.current = body.style.overflow;
    body.style.overflow = 'hidden';
    body.classList.add('crosshook-modal-open');

    hiddenNodesRef.current = Array.from(body.children)
      .filter((child): child is HTMLElement => child instanceof HTMLElement && child !== portalHost)
      .map((element) => {
        const inertState = (element as HTMLElement & { inert?: boolean }).inert ?? false;
        const ariaHidden = element.getAttribute('aria-hidden');
        (element as HTMLElement & { inert?: boolean }).inert = true;
        element.setAttribute('aria-hidden', 'true');
        return { element, inert: inertState, ariaHidden };
      });

    const frame = window.requestAnimationFrame(() => {
      if (focusElement(headingRef.current)) return;
      const focusable = surfaceRef.current ? getFocusableElements(surfaceRef.current) : [];
      if (focusable.length > 0) focusElement(focusable[0]);
    });

    return () => {
      window.cancelAnimationFrame(frame);
      body.style.overflow = bodyStyleRef.current;
      body.classList.remove('crosshook-modal-open');

      for (const { element, inert, ariaHidden } of hiddenNodesRef.current) {
        (element as HTMLElement & { inert?: boolean }).inert = inert;
        if (ariaHidden === null) {
          element.removeAttribute('aria-hidden');
        } else {
          element.setAttribute('aria-hidden', ariaHidden);
        }
      }
      hiddenNodesRef.current = [];

      const restoreTarget = previouslyFocusedRef.current;
      if (restoreTarget && restoreTarget.isConnected) {
        focusElement(restoreTarget);
      }
      previouslyFocusedRef.current = null;
    };
  }, [isMounted]);

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

  /* ── Keyboard handler ── */

  function handleKeyDown(event: ReactKeyboardEvent<HTMLDivElement>) {
    if (event.key === 'Escape') {
      event.stopPropagation();
      event.preventDefault();
      if (pendingRestore) {
        setPendingRestore(null);
        setRestoreError(null);
        return;
      }
      onClose();
      return;
    }

    if (event.key !== 'Tab') return;

    const container = surfaceRef.current;
    if (!container) return;

    const focusable = getFocusableElements(container);
    if (focusable.length === 0) {
      event.preventDefault();
      return;
    }

    const currentIndex = focusable.indexOf(document.activeElement as HTMLElement);
    const lastIndex = focusable.length - 1;

    if (event.shiftKey) {
      if (currentIndex <= 0) {
        event.preventDefault();
        focusElement(focusable[lastIndex]);
      }
      return;
    }

    if (currentIndex === -1 || currentIndex === lastIndex) {
      event.preventDefault();
      focusElement(focusable[0]);
    }
  }

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
          <div
            className="crosshook-history-timeline"
            role="listbox"
            aria-label="Revision history"
            aria-orientation="vertical"
          >
            {revisionsLoading ? (
              <div className="crosshook-history-empty">
                <span className="crosshook-muted">Loading history…</span>
              </div>
            ) : revisionsError ? (
              <div className="crosshook-history-empty">
                <p className="crosshook-danger" style={{ margin: 0 }}>
                  Couldn't load configuration history.
                </p>
                <p className="crosshook-help-text" style={{ marginTop: 6 }}>
                  {revisionsError}
                </p>
              </div>
            ) : revisions.length === 0 ? (
              <div className="crosshook-history-empty">
                <p style={{ margin: 0, fontWeight: 600 }}>No snapshots yet</p>
                <p className="crosshook-help-text" style={{ marginTop: 6 }}>
                  Snapshots are created when you save or when changes are auto-captured.
                </p>
              </div>
            ) : (
              revisions.map((rev) => (
                <button
                  key={rev.id}
                  type="button"
                  role="option"
                  aria-selected={selectedRevision?.id === rev.id}
                  className={
                    'crosshook-history-timeline-item' +
                    (selectedRevision?.id === rev.id ? ' crosshook-history-timeline-item--selected' : '')
                  }
                  onClick={() => selectRevision(rev)}
                >
                  <div className="crosshook-history-timeline-item__header">
                    <span className="crosshook-history-badge">{SOURCE_LABELS[rev.source] ?? rev.source}</span>
                    {rev.is_last_known_working && (
                      <span className="crosshook-history-badge crosshook-history-badge--known-good">Known good</span>
                    )}
                  </div>
                  <div className="crosshook-history-timeline-item__time" title={formatExactDate(rev.created_at)}>
                    {formatRelativeTime(rev.created_at)}
                  </div>
                  {rev.profile_name_at_write !== profileName && (
                    <div className="crosshook-history-timeline-item__oldname crosshook-muted">
                      was: {rev.profile_name_at_write}
                    </div>
                  )}
                </button>
              ))
            )}
          </div>

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
              <div className="crosshook-history-confirm" role="region" aria-label="Restore confirmation">
                <h3 style={{ margin: '0 0 12px' }}>Restore this configuration snapshot?</h3>
                <p className="crosshook-help-text" style={{ marginBottom: 16 }}>
                  You're restoring the snapshot from <strong>{formatExactDate(pendingRestore.created_at)}</strong> (
                  {SOURCE_LABELS[pendingRestore.source] ?? pendingRestore.source}). Your current config will be saved as
                  a new snapshot first.
                </p>
                {restoreError ? (
                  <p className="crosshook-danger" role="alert" style={{ marginBottom: 12 }}>
                    {restoreError}
                  </p>
                ) : null}
                <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap' }}>
                  <button
                    type="button"
                    className="crosshook-button"
                    disabled={restoring}
                    onClick={() => void handleConfirmRestore()}
                  >
                    {restoring ? 'Restoring…' : 'Restore snapshot'}
                  </button>
                  <button
                    type="button"
                    className="crosshook-button crosshook-button--secondary"
                    disabled={restoring}
                    onClick={() => {
                      setPendingRestore(null);
                      setRestoreError(null);
                    }}
                  >
                    Keep current config
                  </button>
                </div>
              </div>
            ) : (
              /* Diff view + actions */
              <>
                <div className="crosshook-history-detail-header">
                  <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap', alignItems: 'center' }}>
                    <span className="crosshook-history-badge">
                      {SOURCE_LABELS[selectedRevision.source] ?? selectedRevision.source}
                    </span>
                    {selectedRevision.is_last_known_working && (
                      <span className="crosshook-history-badge crosshook-history-badge--known-good">Known good</span>
                    )}
                  </div>
                  <div className="crosshook-help-text" style={{ marginTop: 6 }}>
                    {formatExactDate(selectedRevision.created_at)}
                    {selectedRevision.source_revision_id !== null && (
                      <span className="crosshook-muted"> — restored from #{selectedRevision.source_revision_id}</span>
                    )}
                  </div>
                </div>

                <div className="crosshook-history-diff-area">
                  {diffLoading ? (
                    <span className="crosshook-muted">Loading diff…</span>
                  ) : diffError ? (
                    <p className="crosshook-danger" role="alert">
                      {diffError}
                    </p>
                  ) : diff ? (
                    <DiffView diff={diff} />
                  ) : null}
                </div>

                <div className="crosshook-history-detail-actions">
                  <button
                    type="button"
                    className="crosshook-button"
                    onClick={() => {
                      setRestoreSuccess(null);
                      setRestoreError(null);
                      setMarkKnownGoodError(null);
                      setPendingRestore(selectedRevision);
                    }}
                  >
                    Restore snapshot
                  </button>
                  {!selectedRevision.is_last_known_working && (
                    <button
                      type="button"
                      className="crosshook-button crosshook-button--secondary"
                      disabled={markingKnownGoodId === selectedRevision.id}
                      onClick={() => void handleMarkKnownGood(selectedRevision)}
                    >
                      {markingKnownGoodId === selectedRevision.id ? 'Marking…' : 'Mark as known good'}
                    </button>
                  )}
                  {markKnownGoodError ? (
                    <p className="crosshook-danger" role="alert" style={{ margin: '8px 0 0', width: '100%' }}>
                      {markKnownGoodError}
                    </p>
                  ) : null}
                </div>
              </>
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
