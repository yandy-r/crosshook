import { Fragment, type KeyboardEvent, type MouseEvent, useEffect, useId, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import type { ApplyMigrationRequest, BatchMigrationResult, MigrationScanResult, MigrationSuggestion } from '../types';
import { CollapsibleSection } from './ui/CollapsibleSection';
import '../styles/preview.css';

/* ───────── Focus-trap helpers (mirrors LauncherPreviewModal) ───────── */
const RESET_SCROLL_MOMENTUM_EVENT = 'crosshook:reset-scroll-momentum';

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

/* ───────── Helpers ───────── */

const FIELD_LABELS: Record<string, string> = {
  steam_proton_path: 'Steam Proton',
  runtime_proton_path: 'Runtime Proton',
};

function rowKey(s: MigrationSuggestion): string {
  return `${s.profile_name}:${s.field}`;
}

function isSafe(s: MigrationSuggestion): boolean {
  return !s.crosses_major_version && s.confidence >= 0.75;
}

interface ConfidenceInfo {
  text: string;
  color: string;
}

function getConfidenceInfo(s: MigrationSuggestion): ConfidenceInfo {
  if (s.confidence < 0.75) {
    return { text: 'Different family', color: 'var(--crosshook-color-warning)' };
  }
  if (s.crosses_major_version) {
    return { text: 'Major version change', color: 'var(--crosshook-color-warning)' };
  }
  if (s.confidence >= 0.9) {
    return { text: 'Upgrade', color: 'var(--crosshook-color-success)' };
  }
  return { text: 'Older version', color: 'var(--crosshook-color-warning)' };
}

/* ───────── MigrationReviewModal ───────── */

export interface MigrationReviewModalProps {
  scanResult: MigrationScanResult;
  onClose: () => void;
  onApply: (requests: ApplyMigrationRequest[]) => void;
  isBatchApplying: boolean;
  batchResult: BatchMigrationResult | null;
  batchError: string | null;
}

export function MigrationReviewModal({
  scanResult,
  onClose,
  onApply,
  isBatchApplying,
  batchResult,
  batchError,
}: MigrationReviewModalProps) {
  const portalHostRef = useRef<HTMLElement | null>(null);
  const surfaceRef = useRef<HTMLDivElement | null>(null);
  const headingRef = useRef<HTMLHeadingElement | null>(null);
  const previouslyFocusedRef = useRef<HTMLElement | null>(null);
  const bodyStyleRef = useRef('');
  const hiddenNodesRef = useRef<Array<{ element: HTMLElement; inert: boolean; ariaHidden: string | null }>>([]);
  const selectAllRef = useRef<HTMLInputElement>(null);
  const titleId = useId();
  const [isMounted, setIsMounted] = useState(false);

  const safeRows = scanResult.suggestions.filter(isSafe);
  const needsReviewRows = scanResult.suggestions.filter((s) => !isSafe(s));

  const [checked, setChecked] = useState<Set<string>>(() => new Set(safeRows.map(rowKey)));

  const isAllSafeChecked = safeRows.length > 0 && safeRows.every((s) => checked.has(rowKey(s)));
  const isSomeSafeChecked = safeRows.some((s) => checked.has(rowKey(s)));
  const selectedCount = scanResult.suggestions.filter((s) => checked.has(rowKey(s))).length;

  useEffect(() => {
    if (selectAllRef.current) {
      selectAllRef.current.indeterminate = !isAllSafeChecked && isSomeSafeChecked;
    }
  }, [isAllSafeChecked, isSomeSafeChecked]);

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

  useEffect(() => {
    if (!isMounted) return;

    const { body } = document;
    const portalHost = portalHostRef.current;
    if (!portalHost) return;

    window.dispatchEvent(new Event(RESET_SCROLL_MOMENTUM_EVENT));

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
      window.dispatchEvent(new Event(RESET_SCROLL_MOMENTUM_EVENT));
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
      if (restoreTarget?.isConnected) {
        focusElement(restoreTarget);
      }
      previouslyFocusedRef.current = null;
    };
  }, [isMounted]);

  function handleKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === 'Escape') {
      event.stopPropagation();
      event.preventDefault();
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

  function handleBackdropMouseDown(event: MouseEvent<HTMLDivElement>) {
    if (event.target !== event.currentTarget) return;
    onClose();
  }

  function handleSelectAll() {
    if (isAllSafeChecked) {
      setChecked((prev) => {
        const next = new Set(prev);
        for (const s of safeRows) next.delete(rowKey(s));
        return next;
      });
    } else {
      setChecked((prev) => {
        const next = new Set(prev);
        for (const s of safeRows) next.add(rowKey(s));
        return next;
      });
    }
  }

  function handleCheckRow(key: string, isChecked: boolean) {
    setChecked((prev) => {
      const next = new Set(prev);
      if (isChecked) {
        next.add(key);
      } else {
        next.delete(key);
      }
      return next;
    });
  }

  function handleConfirm() {
    const requests: ApplyMigrationRequest[] = scanResult.suggestions
      .filter((s) => checked.has(rowKey(s)))
      .map((s) => ({ profile_name: s.profile_name, field: s.field, new_path: s.new_path }));
    onApply(requests);
  }

  if (!isMounted || !portalHostRef.current) return null;

  return createPortal(
    <div className="crosshook-modal" role="presentation">
      <div className="crosshook-modal__backdrop" aria-hidden="true" onMouseDown={handleBackdropMouseDown} />
      <div
        ref={surfaceRef}
        className="crosshook-modal__surface crosshook-panel crosshook-focus-scope"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        data-crosshook-focus-root="modal"
        onKeyDown={handleKeyDown}
      >
        {/* Header */}
        <header className="crosshook-modal__header">
          <div className="crosshook-modal__heading-block">
            <div className="crosshook-heading-eyebrow">Migration</div>
            <h2 ref={headingRef} id={titleId} className="crosshook-modal__title" tabIndex={-1}>
              Fix Proton Paths
              {!isBatchApplying && !batchResult && scanResult.affected_count > 0 && (
                <span className="crosshook-muted" style={{ fontSize: '0.75em', fontWeight: 400, marginLeft: '8px' }}>
                  ({scanResult.affected_count} affected)
                </span>
              )}
            </h2>
          </div>
        </header>

        {/* Body */}
        <div className="crosshook-modal__body" style={{ gridRow: 3 }}>
          {/* Applying phase */}
          {isBatchApplying && (
            <div aria-live="polite" style={{ padding: '16px 0' }}>
              <p>
                Updating {selectedCount} profile{selectedCount !== 1 ? 's' : ''}\u2026
              </p>
              {selectedCount >= 3 && (
                <progress
                  aria-label="Updating profiles"
                  style={{
                    width: '100%',
                    marginTop: '12px',
                    accentColor: 'var(--crosshook-color-accent)',
                  }}
                />
              )}
            </div>
          )}

          {/* Batch apply error */}
          {!isBatchApplying && batchError && (
            <div role="alert" style={{ color: 'var(--crosshook-color-danger)', marginBottom: '12px' }}>
              {batchError}
            </div>
          )}

          {/* Result phase */}
          {!isBatchApplying && batchResult && (
            <div role="status" style={{ padding: '16px 0' }}>
              <p style={{ color: 'var(--crosshook-color-success)', fontWeight: 600 }}>
                &#10003; {batchResult.applied_count} profile{batchResult.applied_count !== 1 ? 's' : ''} updated.
                {batchResult.failed_count > 0 && (
                  <span style={{ color: 'var(--crosshook-color-danger)', marginLeft: '8px' }}>
                    {batchResult.failed_count} failed.
                  </span>
                )}
                {batchResult.skipped_count > 0 && (
                  <span className="crosshook-muted" style={{ marginLeft: '8px' }}>
                    {batchResult.skipped_count} skipped.
                  </span>
                )}
              </p>
              {batchResult.failed_count > 0 && (
                <ul style={{ marginTop: '8px', paddingLeft: '16px' }}>
                  {batchResult.results
                    .filter((r) => r.outcome === 'failed')
                    .map((r) => (
                      <li
                        key={`${r.profile_name}:${r.field}`}
                        style={{ color: 'var(--crosshook-color-danger)', fontSize: '0.875em' }}
                      >
                        <strong>{r.profile_name}</strong>: {r.error ?? 'Unknown error'}
                      </li>
                    ))}
                </ul>
              )}
            </div>
          )}

          {/* Review phase */}
          {!isBatchApplying && !batchResult && (
            <div className="crosshook-preview-modal__sections">
              {/* Safe rows (pre-checked) */}
              {safeRows.length > 0 && (
                <div style={{ marginBottom: '8px' }}>
                  <div
                    style={{
                      display: 'flex',
                      alignItems: 'center',
                      gap: '8px',
                      padding: '8px 0',
                      borderBottom: '1px solid var(--crosshook-color-border)',
                      marginBottom: '4px',
                    }}
                  >
                    <label
                      style={{
                        display: 'flex',
                        alignItems: 'center',
                        gap: '8px',
                        cursor: 'pointer',
                        minHeight: 'var(--crosshook-touch-target-min)',
                      }}
                      className="crosshook-focus-ring crosshook-nav-target"
                    >
                      <input
                        ref={selectAllRef}
                        type="checkbox"
                        checked={isAllSafeChecked}
                        onChange={handleSelectAll}
                        aria-label="Select all safe migration suggestions"
                      />
                      <span>Select All ({safeRows.length})</span>
                    </label>
                  </div>
                  <MigrationTable rows={safeRows} checked={checked} onCheckRow={handleCheckRow} />
                </div>
              )}

              {/* Needs manual review (collapsed, unchecked) */}
              {needsReviewRows.length > 0 && (
                <CollapsibleSection title={`Needs Manual Review (${needsReviewRows.length})`} defaultOpen={false}>
                  <p className="crosshook-muted" style={{ fontSize: '0.875em', marginBottom: '8px' }}>
                    These suggestions involve cross-major or cross-family Proton changes. Review carefully — your WINE
                    prefix may need recreation.
                  </p>
                  <MigrationTable rows={needsReviewRows} checked={checked} onCheckRow={handleCheckRow} showWarnings />
                </CollapsibleSection>
              )}

              {/* No-suggestion profiles */}
              {scanResult.unmatched.length > 0 && (
                <div
                  style={{
                    marginTop: '12px',
                    padding: '12px',
                    borderRadius: 'var(--crosshook-radius-sm)',
                    border: '1px solid var(--crosshook-color-border)',
                  }}
                >
                  <p className="crosshook-muted" style={{ fontSize: '0.875em', marginBottom: '8px' }}>
                    The following profiles have no Proton installation available. Fix manually.
                  </p>
                  <ul style={{ paddingLeft: '16px' }}>
                    {scanResult.unmatched.map((u) => (
                      <li key={`${u.profile_name}:${u.field}`} style={{ fontSize: '0.875em', marginBottom: '4px' }}>
                        <strong>{u.profile_name}</strong>
                        <span className="crosshook-muted" style={{ marginLeft: '8px' }}>
                          {FIELD_LABELS[u.field] ?? u.field}
                        </span>
                        <code
                          style={{
                            display: 'block',
                            color: 'var(--crosshook-color-danger)',
                            fontSize: '0.85em',
                            wordBreak: 'break-all',
                          }}
                        >
                          {u.stale_path}
                        </code>
                      </li>
                    ))}
                  </ul>
                </div>
              )}

              {scanResult.suggestions.length === 0 && scanResult.unmatched.length === 0 && (
                <p className="crosshook-muted">No stale Proton paths found.</p>
              )}
            </div>
          )}
        </div>

        {/* Controller prompts — shown during review phase only */}
        {!isBatchApplying && !batchResult && (
          <div
            aria-hidden="true"
            style={{
              display: 'flex',
              justifyContent: 'center',
              padding: '8px 0 0',
              gridRow: 'unset',
            }}
          >
            <div className="crosshook-controller-prompts__surface">
              <span className="crosshook-controller-prompts__item">
                <span className="crosshook-controller-prompts__glyph">A</span>
                <span className="crosshook-controller-prompts__label">Toggle</span>
              </span>
              <span className="crosshook-controller-prompts__item">
                <span className="crosshook-controller-prompts__glyph">B</span>
                <span className="crosshook-controller-prompts__label">Cancel</span>
              </span>
              <span className="crosshook-controller-prompts__item">
                <span className="crosshook-controller-prompts__glyph">Start</span>
                <span className="crosshook-controller-prompts__label">Confirm</span>
              </span>
            </div>
          </div>
        )}

        {/* Footer */}
        <footer className="crosshook-modal__footer" style={{ gridRow: 4 }}>
          <span />
          <div className="crosshook-modal__footer-actions">
            {batchResult ? (
              <button
                type="button"
                className="crosshook-button crosshook-focus-ring crosshook-nav-target"
                style={{ minHeight: 'var(--crosshook-touch-target-min)' }}
                onClick={onClose}
              >
                Close
              </button>
            ) : (
              <>
                <button
                  type="button"
                  className="crosshook-button crosshook-button--ghost crosshook-focus-ring crosshook-nav-target"
                  style={{ minHeight: 'var(--crosshook-touch-target-min)' }}
                  onClick={onClose}
                  disabled={isBatchApplying}
                >
                  Cancel
                </button>
                <button
                  type="button"
                  className="crosshook-button crosshook-focus-ring crosshook-nav-target"
                  style={{ minHeight: 'var(--crosshook-touch-target-min)' }}
                  onClick={handleConfirm}
                  disabled={selectedCount === 0 || isBatchApplying}
                >
                  {isBatchApplying
                    ? 'Updating\u2026'
                    : `Update ${selectedCount} Profile${selectedCount !== 1 ? 's' : ''}`}
                </button>
              </>
            )}
          </div>
        </footer>
      </div>
    </div>,
    portalHostRef.current
  );
}

/* ───────── MigrationTable sub-component ───────── */

function MigrationTable({
  rows,
  checked,
  onCheckRow,
  showWarnings = false,
}: {
  rows: MigrationSuggestion[];
  checked: Set<string>;
  onCheckRow: (key: string, isChecked: boolean) => void;
  showWarnings?: boolean;
}) {
  return (
    <table className="crosshook-health-dashboard-table" style={{ width: '100%', fontSize: '0.875em' }}>
      <thead>
        <tr>
          <th scope="col" style={{ width: '32px' }}></th>
          <th scope="col">Profile</th>
          <th scope="col">Field</th>
          <th scope="col">Current</th>
          <th scope="col">Suggested</th>
          <th scope="col">Confidence</th>
        </tr>
      </thead>
      <tbody>
        {rows.map((s) => {
          const key = rowKey(s);
          const isChecked = checked.has(key);
          const badge = getConfidenceInfo(s);
          return (
            <Fragment key={key}>
              <tr>
                <td>
                  <input
                    type="checkbox"
                    checked={isChecked}
                    onChange={(e) => onCheckRow(key, e.target.checked)}
                    className="crosshook-focus-ring crosshook-nav-target crosshook-focus-target"
                    aria-label={`Select ${s.profile_name} (${FIELD_LABELS[s.field] ?? s.field}) for migration`}
                    style={{ minHeight: 'var(--crosshook-touch-target-min)' }}
                  />
                </td>
                <td>{s.profile_name}</td>
                <td className="crosshook-muted">{FIELD_LABELS[s.field] ?? s.field}</td>
                <td>
                  <span style={{ color: 'var(--crosshook-color-danger)' }} title={s.old_path}>
                    {s.old_proton_name}
                  </span>
                </td>
                <td>
                  <span style={{ color: 'var(--crosshook-color-success)' }} title={s.new_path}>
                    {s.new_proton_name}
                  </span>
                </td>
                <td>
                  <span style={{ color: badge.color, fontWeight: 600 }}>{badge.text}</span>
                </td>
              </tr>
              <tr>
                <td colSpan={6} style={{ padding: '0 0 4px 32px' }}>
                  <CollapsibleSection title="Show full path" defaultOpen={false}>
                    <div style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                      <code
                        style={{
                          color: 'var(--crosshook-color-danger)',
                          wordBreak: 'break-all',
                          fontSize: '0.85em',
                        }}
                      >
                        {s.old_path}
                      </code>
                      <span aria-hidden="true" style={{ color: 'var(--crosshook-color-text-muted)' }}>
                        &darr;
                      </span>
                      <code
                        style={{
                          color: 'var(--crosshook-color-success)',
                          wordBreak: 'break-all',
                          fontSize: '0.85em',
                        }}
                      >
                        {s.new_path}
                      </code>
                    </div>
                  </CollapsibleSection>
                  {showWarnings && s.crosses_major_version && (
                    <div
                      role="alert"
                      style={{
                        color: 'var(--crosshook-color-warning)',
                        fontSize: '0.8em',
                        marginTop: '4px',
                      }}
                    >
                      &#9888; Major version change — your WINE prefix may need recreation
                    </div>
                  )}
                  {showWarnings && s.confidence < 0.75 && (
                    <div
                      role="alert"
                      style={{
                        color: 'var(--crosshook-color-warning)',
                        fontSize: '0.8em',
                        marginTop: '4px',
                      }}
                    >
                      &#9888; Different Proton family — verify compatibility before applying
                    </div>
                  )}
                </td>
              </tr>
            </Fragment>
          );
        })}
      </tbody>
    </table>
  );
}

export default MigrationReviewModal;
