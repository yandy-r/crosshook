import { useEffect, useId, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import type { LutrisImportEntry, LutrisImportPreview, LutrisImportResult } from '../hooks/useLutrisImport';
import { useMigrationReviewFocusTrap } from './migration-review/useMigrationReviewFocusTrap';
import '../styles/preview.css';

function entryKey(entry: LutrisImportEntry): string {
  return entry.source_path;
}

function formatMappedSummary(entry: LutrisImportEntry): string {
  const { mapped } = entry;
  const parts = [
    mapped.launch.method ? `Launch: ${mapped.launch.method}` : null,
    mapped.game.executable_path ? `Exe: ${mapped.game.executable_path}` : null,
    mapped.runtime.prefix_path ? `Prefix: ${mapped.runtime.prefix_path}` : null,
    mapped.runtime.proton_path ? `Proton: ${mapped.runtime.proton_path}` : null,
  ].filter(Boolean);

  return parts.length > 0 ? parts.join(' · ') : 'No mapped fields';
}

export interface LutrisImportModalProps {
  preview: LutrisImportPreview;
  onClose: () => void;
  onImport: (entries: LutrisImportEntry[]) => void;
  isImporting: boolean;
  importResult: LutrisImportResult | null;
  importError: string | null;
}

export function LutrisImportModal({
  preview,
  onClose,
  onImport,
  isImporting,
  importResult,
  importError,
}: LutrisImportModalProps) {
  const { headingRef, surfaceRef, portalHostRef, isMounted, handleBackdropMouseDown, handleKeyDown } =
    useMigrationReviewFocusTrap(onClose);
  const selectAllRef = useRef<HTMLInputElement>(null);
  const titleId = useId();

  const importableEntries = preview.entries.filter((entry) => entry.importable);
  const skippedEntries = preview.entries.filter((entry) => !entry.importable);

  const [checked, setChecked] = useState<Set<string>>(() => new Set(importableEntries.map(entryKey)));

  const isAllImportableChecked =
    importableEntries.length > 0 && importableEntries.every((entry) => checked.has(entryKey(entry)));
  const isSomeImportableChecked = importableEntries.some((entry) => checked.has(entryKey(entry)));
  const selectedCount = preview.entries.filter((entry) => checked.has(entryKey(entry))).length;

  useEffect(() => {
    if (selectAllRef.current) {
      selectAllRef.current.indeterminate = !isAllImportableChecked && isSomeImportableChecked;
    }
  }, [isAllImportableChecked, isSomeImportableChecked]);

  function handleSelectAll() {
    if (isAllImportableChecked) {
      setChecked((prev) => {
        const next = new Set(prev);
        for (const entry of importableEntries) {
          next.delete(entryKey(entry));
        }
        return next;
      });
    } else {
      setChecked((prev) => {
        const next = new Set(prev);
        for (const entry of importableEntries) {
          next.add(entryKey(entry));
        }
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
    const selected = preview.entries.filter((entry) => checked.has(entryKey(entry)));
    onImport(selected);
  }

  if (!isMounted || !portalHostRef.current) {
    return null;
  }

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
        <header className="crosshook-modal__header">
          <div className="crosshook-modal__heading-block">
            <div className="crosshook-heading-eyebrow">Migration</div>
            <h2 ref={headingRef} id={titleId} className="crosshook-modal__title" tabIndex={-1}>
              Import from Lutris
              {!isImporting && !importResult && preview.entries.length > 0 && (
                <span className="crosshook-muted" style={{ fontSize: '0.75em', fontWeight: 400, marginLeft: '8px' }}>
                  ({preview.entries.length} game{preview.entries.length !== 1 ? 's' : ''})
                </span>
              )}
            </h2>
            {preview.lutris_root ? (
              <p
                className="crosshook-muted crosshook-modal__subtitle"
                style={{ margin: '4px 0 0', fontSize: '0.875em' }}
              >
                Source: {preview.lutris_root}
              </p>
            ) : null}
          </div>
        </header>

        <div className="crosshook-modal__body" style={{ gridRow: 3 }}>
          {isImporting && (
            <div aria-live="polite" style={{ padding: '16px 0' }}>
              <p>
                Importing {selectedCount} profile{selectedCount !== 1 ? 's' : ''}&hellip;
              </p>
              {selectedCount >= 3 && (
                <progress
                  aria-label="Importing profiles"
                  style={{
                    width: '100%',
                    marginTop: '12px',
                    accentColor: 'var(--crosshook-color-accent)',
                  }}
                />
              )}
            </div>
          )}

          {!isImporting && importError && (
            <div role="alert" style={{ color: 'var(--crosshook-color-danger)', marginBottom: '12px' }}>
              {importError}
            </div>
          )}

          {!isImporting && importResult && (
            <div role="status" style={{ padding: '16px 0' }}>
              <p style={{ color: 'var(--crosshook-color-success)', fontWeight: 600 }}>
                &#10003; {importResult.imported_count} profile{importResult.imported_count !== 1 ? 's' : ''} imported.
                {importResult.failed_count > 0 && (
                  <span style={{ color: 'var(--crosshook-color-danger)', marginLeft: '8px' }}>
                    {importResult.failed_count} failed.
                  </span>
                )}
                {importResult.skipped_count > 0 && (
                  <span className="crosshook-muted" style={{ marginLeft: '8px' }}>
                    {importResult.skipped_count} skipped.
                  </span>
                )}
              </p>
              {importResult.failed_count > 0 && (
                <ul style={{ marginTop: '8px', paddingLeft: '16px' }}>
                  {importResult.results
                    .filter((result) => result.outcome === 'failed')
                    .map((result) => (
                      <li
                        key={entryKey(result.entry)}
                        style={{ color: 'var(--crosshook-color-danger)', fontSize: '0.875em' }}
                      >
                        <strong>{result.entry.game_name}</strong>: {result.error ?? 'Unknown error'}
                      </li>
                    ))}
                </ul>
              )}
            </div>
          )}

          {!isImporting && !importResult && (
            <div className="crosshook-preview-modal__sections">
              {preview.diagnostics.length > 0 && (
                <div
                  style={{
                    marginBottom: '12px',
                    padding: '12px',
                    borderRadius: 'var(--crosshook-radius-sm)',
                    border: '1px solid var(--crosshook-color-border)',
                  }}
                >
                  {preview.diagnostics.map((diagnostic) => (
                    <p key={diagnostic} className="crosshook-muted" style={{ fontSize: '0.875em', margin: '0 0 4px' }}>
                      {diagnostic}
                    </p>
                  ))}
                </div>
              )}

              {importableEntries.length > 0 && (
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
                        checked={isAllImportableChecked}
                        onChange={handleSelectAll}
                        aria-label="Select all importable Lutris games"
                      />
                      <span>Select All ({importableEntries.length})</span>
                    </label>
                  </div>
                  <ul style={{ listStyle: 'none', margin: 0, padding: 0 }}>
                    {importableEntries.map((entry) => {
                      const key = entryKey(entry);
                      return (
                        <li
                          key={key}
                          style={{
                            padding: '10px 0',
                            borderBottom: '1px solid var(--crosshook-color-border)',
                          }}
                        >
                          <label
                            style={{
                              display: 'flex',
                              alignItems: 'flex-start',
                              gap: '10px',
                              cursor: 'pointer',
                            }}
                            className="crosshook-focus-ring crosshook-nav-target"
                          >
                            <input
                              type="checkbox"
                              checked={checked.has(key)}
                              onChange={(event) => handleCheckRow(key, event.target.checked)}
                              aria-label={`Import ${entry.game_name}`}
                              style={{ marginTop: '4px' }}
                            />
                            <span style={{ flex: 1, minWidth: 0 }}>
                              <span style={{ display: 'block', fontWeight: 600 }}>{entry.game_name}</span>
                              <span className="crosshook-muted" style={{ display: 'block', fontSize: '0.875em' }}>
                                {entry.suggested_name} · {entry.runner}
                              </span>
                              <span
                                className="crosshook-muted"
                                style={{ display: 'block', fontSize: '0.8125em', wordBreak: 'break-all' }}
                              >
                                {formatMappedSummary(entry)}
                              </span>
                              {entry.warnings.length > 0 && (
                                <span
                                  style={{
                                    display: 'block',
                                    marginTop: '6px',
                                    fontSize: '0.8125em',
                                    color: 'var(--crosshook-color-warning, var(--crosshook-color-accent))',
                                  }}
                                >
                                  {entry.warnings.join(' · ')}
                                </span>
                              )}
                            </span>
                          </label>
                        </li>
                      );
                    })}
                  </ul>
                </div>
              )}

              {skippedEntries.length > 0 && (
                <div
                  style={{
                    marginTop: '12px',
                    padding: '12px',
                    borderRadius: 'var(--crosshook-radius-sm)',
                    border: '1px solid var(--crosshook-color-border)',
                  }}
                >
                  <p style={{ fontWeight: 600, marginBottom: '8px' }}>Not importable ({skippedEntries.length})</p>
                  <ul style={{ listStyle: 'none', margin: 0, padding: 0 }}>
                    {skippedEntries.map((entry) => (
                      <li key={entryKey(entry)} style={{ marginBottom: '8px', fontSize: '0.875em' }}>
                        <strong>{entry.game_name}</strong>
                        <span className="crosshook-muted" style={{ marginLeft: '8px' }}>
                          {entry.runner}
                        </span>
                        {entry.warnings.length > 0 && (
                          <span
                            style={{
                              display: 'block',
                              marginTop: '4px',
                              color: 'var(--crosshook-color-warning, var(--crosshook-color-accent))',
                            }}
                          >
                            {entry.warnings.join(' · ')}
                          </span>
                        )}
                      </li>
                    ))}
                  </ul>
                </div>
              )}

              {preview.entries.length === 0 && (
                <p className="crosshook-muted">No Lutris games were found in the selected directory.</p>
              )}
            </div>
          )}
        </div>

        <footer className="crosshook-modal__footer" style={{ gridRow: 4 }}>
          <span />
          <div className="crosshook-modal__footer-actions">
            {importResult ? (
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
                  disabled={isImporting}
                >
                  Cancel
                </button>
                <button
                  type="button"
                  className="crosshook-button crosshook-focus-ring crosshook-nav-target"
                  style={{ minHeight: 'var(--crosshook-touch-target-min)' }}
                  onClick={handleConfirm}
                  disabled={selectedCount === 0 || isImporting}
                >
                  {isImporting ? 'Importing\u2026' : `Import ${selectedCount} Profile${selectedCount !== 1 ? 's' : ''}`}
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

export default LutrisImportModal;
