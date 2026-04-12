import { type MouseEvent, useCallback, useEffect, useId, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { useFocusTrap } from '@/hooks/useFocusTrap';
import type { CollectionImportPreview } from '@/types/collections';
import { isCollectionDefaultsEmpty } from '@/types/profile';

import './CollectionImportReviewModal.css';

const SKIP_VALUE = '__skip__';

export interface CollectionImportReviewModalProps {
  open: boolean;
  preview: CollectionImportPreview | null;
  applying: boolean;
  /** Shown inside the dialog when import/apply fails (e.g. from applyImportedCollection). */
  importSessionError?: string | null;
  onClose: () => void;
  onConfirm: (input: { name: string; description: string | null; ambiguousResolutions: (string | null)[] }) => void;
}

export function CollectionImportReviewModal({
  open,
  preview,
  applying,
  importSessionError = null,
  onClose,
  onConfirm,
}: CollectionImportReviewModalProps) {
  const titleId = useId();
  const descriptionId = useId();
  const surfaceRef = useRef<HTMLDivElement | null>(null);
  const headingRef = useRef<HTMLHeadingElement | null>(null);
  const portalHostRef = useRef<HTMLElement | null>(null);
  const [isMounted, setIsMounted] = useState(false);

  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [ambiguousSelect, setAmbiguousSelect] = useState<string[]>([]);

  useEffect(() => {
    if (!open || !preview) {
      return;
    }
    setName(preview.manifest.name);
    setDescription(preview.manifest.description?.trim() ?? '');
    setAmbiguousSelect(preview.ambiguous.map(() => ''));
  }, [open, preview]);

  useEffect(() => {
    if (typeof document === 'undefined') {
      return;
    }
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

  const guardedOnClose = useCallback(() => {
    if (!applying) onClose();
  }, [applying, onClose]);

  const { handleKeyDown } = useFocusTrap({
    open: open && preview !== null,
    panelRef: surfaceRef,
    onClose: guardedOnClose,
    initialFocusRef: headingRef,
  });

  const ambiguousReady =
    preview !== null &&
    (preview.ambiguous.length === 0 ||
      (ambiguousSelect.length === preview.ambiguous.length && ambiguousSelect.every((v) => v !== '')));

  const canConfirm = preview !== null && name.trim() !== '' && ambiguousReady && !applying;

  const handleConfirm = useCallback(() => {
    if (!preview || !canConfirm) {
      return;
    }
    const ambiguousResolutions: (string | null)[] = ambiguousSelect.map((v) => (v === SKIP_VALUE ? null : v));
    onConfirm({
      name: name.trim(),
      description: description.trim() === '' ? null : description.trim(),
      ambiguousResolutions,
    });
  }, [ambiguousSelect, canConfirm, description, name, onConfirm, preview]);

  function handleBackdropMouseDown(event: MouseEvent<HTMLDivElement>) {
    if (event.target !== event.currentTarget) {
      return;
    }
    if (applying) {
      event.preventDefault();
      event.stopPropagation?.();
      return;
    }
    onClose();
  }

  if (!open || !preview || !isMounted || !portalHostRef.current) {
    return null;
  }

  const defaults = preview.manifest.defaults;
  const hasDefaults = defaults !== undefined && defaults !== null && !isCollectionDefaultsEmpty(defaults);

  return createPortal(
    <div className="crosshook-modal" role="presentation">
      <div className="crosshook-modal__backdrop" aria-hidden="true" onMouseDown={handleBackdropMouseDown} />
      <div
        ref={surfaceRef}
        className="crosshook-modal__surface crosshook-panel crosshook-focus-scope crosshook-collection-import-review"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={descriptionId}
        data-crosshook-focus-root="modal"
        onKeyDown={handleKeyDown}
      >
        <header className="crosshook-modal__header">
          <div className="crosshook-modal__heading-block">
            <h2 ref={headingRef} id={titleId} className="crosshook-modal__title" tabIndex={-1}>
              Import collection preset
            </h2>
            <p id={descriptionId} className="crosshook-modal__description">
              Review matches before creating a new collection. Unmatched descriptors are skipped.
            </p>
          </div>
          <button
            type="button"
            className="crosshook-button crosshook-button--ghost crosshook-modal__close"
            data-crosshook-modal-close
            disabled={applying}
            onClick={onClose}
          >
            Close
          </button>
        </header>

        <form
          className="crosshook-modal__body crosshook-collection-import-review__body"
          onSubmit={(e) => {
            e.preventDefault();
            handleConfirm();
          }}
        >
          {importSessionError ? (
            <div className="crosshook-collection-import-review__alert" role="alert">
              {importSessionError}
            </div>
          ) : null}
          <div className="crosshook-collection-import-review__field">
            <label className="crosshook-label" htmlFor={`${titleId}-name`}>
              Collection name
            </label>
            <input
              id={`${titleId}-name`}
              type="text"
              className="crosshook-input"
              value={name}
              onChange={(e) => setName(e.target.value)}
              disabled={applying}
            />
          </div>
          <div className="crosshook-collection-import-review__field">
            <label className="crosshook-label" htmlFor={`${titleId}-desc`}>
              Description (optional)
            </label>
            <textarea
              id={`${titleId}-desc`}
              className="crosshook-input crosshook-collection-import-review__textarea"
              rows={2}
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              disabled={applying}
            />
          </div>

          <ul className="crosshook-collection-import-review__stats" aria-label="Import summary">
            <li>Matched: {preview.matched.length}</li>
            <li>Ambiguous: {preview.ambiguous.length}</li>
            <li>Unmatched: {preview.unmatched.length}</li>
            <li>Defaults: {hasDefaults ? 'included' : 'none'}</li>
          </ul>

          {preview.matched.length > 0 ? (
            <section className="crosshook-collection-import-review__section" aria-label="Matched profiles">
              <h3 className="crosshook-collection-import-review__section-title">Matched</h3>
              <ul className="crosshook-collection-import-review__list">
                {preview.matched.map((m, i) => (
                  <li
                    key={`${m.local_profile_name}-${m.descriptor.steam_app_id || m.descriptor.trainer_community_trainer_sha256 || i}`}
                  >
                    <strong>{m.local_profile_name}</strong>
                    <span className="crosshook-collection-import-review__muted">
                      {' '}
                      — {m.descriptor.game_name || m.descriptor.steam_app_id || 'profile'}
                    </span>
                  </li>
                ))}
              </ul>
            </section>
          ) : null}

          {preview.ambiguous.length > 0 ? (
            <section className="crosshook-collection-import-review__section" aria-label="Ambiguous matches">
              <h3 className="crosshook-collection-import-review__section-title">Choose local profile</h3>
              {preview.ambiguous.map((row, i) => (
                <div
                  key={`${row.descriptor.steam_app_id}-${row.descriptor.trainer_community_trainer_sha256}-${i}`}
                  className="crosshook-collection-import-review__ambiguous"
                >
                  <p className="crosshook-collection-import-review__descriptor">
                    {row.descriptor.game_name || row.descriptor.steam_app_id || 'Unknown'}{' '}
                    <span className="crosshook-collection-import-review__muted">
                      (steam_app_id: {row.descriptor.steam_app_id || '—'})
                    </span>
                  </p>
                  <label className="crosshook-label" htmlFor={`${titleId}-amb-${i}`}>
                    Resolve
                  </label>
                  <select
                    id={`${titleId}-amb-${i}`}
                    className="crosshook-input"
                    value={ambiguousSelect[i] ?? ''}
                    onChange={(e) => {
                      const v = e.target.value;
                      setAmbiguousSelect((prev) => {
                        const next = [...prev];
                        next[i] = v;
                        return next;
                      });
                    }}
                    disabled={applying}
                  >
                    <option value="">Choose…</option>
                    <option value={SKIP_VALUE}>Skip this entry</option>
                    {row.candidates.map((c) => (
                      <option key={c.profile_name} value={c.profile_name}>
                        {c.profile_name} — {c.game_name || c.steam_app_id}
                      </option>
                    ))}
                  </select>
                </div>
              ))}
            </section>
          ) : null}

          {preview.unmatched.length > 0 ? (
            <section className="crosshook-collection-import-review__section" aria-label="Unmatched descriptors">
              <h3 className="crosshook-collection-import-review__section-title">Unmatched (will be skipped)</h3>
              <ul className="crosshook-collection-import-review__list">
                {preview.unmatched.map((u, i) => (
                  <li key={`${u.steam_app_id}-${u.trainer_community_trainer_sha256}-${i}`}>
                    {u.game_name || u.steam_app_id || 'Descriptor'} — steam {u.steam_app_id || '—'}
                  </li>
                ))}
              </ul>
            </section>
          ) : null}

          <footer className="crosshook-modal__footer">
            <div className="crosshook-modal__footer-actions">
              <button
                type="button"
                className="crosshook-button crosshook-button--ghost"
                data-crosshook-modal-close
                onClick={onClose}
                disabled={applying}
              >
                Cancel
              </button>
              <button type="submit" className="crosshook-button" disabled={!canConfirm}>
                Import collection
              </button>
            </div>
          </footer>
        </form>
      </div>
    </div>,
    portalHostRef.current
  );
}
