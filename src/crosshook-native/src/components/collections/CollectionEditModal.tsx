import { createPortal } from 'react-dom';
import { useCallback, useEffect, useId, useRef, useState, type FormEvent, type MouseEvent } from 'react';

import { useFocusTrap } from '@/hooks/useFocusTrap';

export type CollectionEditMode = 'create' | 'edit';

export interface CollectionEditModalProps {
  open: boolean;
  mode: CollectionEditMode;
  initialName?: string;
  initialDescription?: string | null;
  onClose: () => void;
  onSubmitCreate: (name: string, description: string | null) => Promise<boolean>;
  onSubmitEdit: (name: string, description: string | null) => Promise<boolean>;
  /** Hook or IPC error surfaced by parent (e.g. duplicate name). */
  externalError?: string | null;
}

export function CollectionEditModal({
  open,
  mode,
  initialName = '',
  initialDescription = null,
  onClose,
  onSubmitCreate,
  onSubmitEdit,
  externalError,
}: CollectionEditModalProps) {
  const titleId = useId();
  const descriptionId = useId();
  const panelRef = useRef<HTMLDivElement>(null);
  const headingRef = useRef<HTMLHeadingElement>(null);
  const [name, setName] = useState(initialName);
  const [description, setDescription] = useState(initialDescription ?? '');
  const [busy, setBusy] = useState(false);
  const [localError, setLocalError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) {
      return;
    }
    setName(initialName);
    setDescription(initialDescription ?? '');
    setLocalError(null);
  }, [open, initialName, initialDescription]);

  const guardedOnClose = useCallback(() => {
    if (!busy) onClose();
  }, [busy, onClose]);

  const { handleKeyDown } = useFocusTrap({
    open,
    panelRef,
    onClose: guardedOnClose,
    initialFocusRef: headingRef,
    restoreFocusOnClose: true,
  });

  const handleSubmit = useCallback(
    async (event: FormEvent) => {
      event.preventDefault();
      const trimmed = name.trim();
      if (!trimmed) {
        setLocalError('Collection name is required.');
        return;
      }
      setBusy(true);
      setLocalError(null);
      try {
        const descNormalized = description.trim() ? description.trim() : null;
        const ok =
          mode === 'create'
            ? await onSubmitCreate(trimmed, descNormalized)
            : await onSubmitEdit(trimmed, descNormalized);
        if (ok) {
          onClose();
        }
      } finally {
        setBusy(false);
      }
    },
    [description, mode, name, onClose, onSubmitCreate, onSubmitEdit]
  );

  const combinedError = localError ?? externalError ?? null;

  if (!open) {
    return null;
  }

  const title = mode === 'create' ? 'Create collection' : 'Edit collection';

  const node = (
    <div className="crosshook-modal" role="presentation">
      <div
        className="crosshook-modal__backdrop"
        aria-hidden="true"
        onMouseDown={(e: MouseEvent<HTMLDivElement>) => {
          if (e.target === e.currentTarget && !busy) {
            onClose();
          }
        }}
      />
      <div
        ref={panelRef}
        className="crosshook-modal__surface crosshook-panel crosshook-focus-scope crosshook-collection-edit-modal"
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
              {title}
            </h2>
          </div>
          <div className="crosshook-modal__header-actions">
            <button
              type="button"
              className="crosshook-button crosshook-button--ghost crosshook-modal__close"
              data-crosshook-modal-close
              disabled={busy}
              onClick={onClose}
            >
              Close
            </button>
          </div>
        </header>
        <form className="crosshook-modal__body" onSubmit={(e) => void handleSubmit(e)}>
          <p id={descriptionId} className="crosshook-muted" style={{ fontSize: '0.85rem', margin: 0 }}>
            {mode === 'create' ? 'Enter a name for your new collection.' : 'Update the collection details.'}
          </p>
          <div className="crosshook-collection-edit-modal__fields">
            <label className="crosshook-label" htmlFor={`${titleId}-name`}>
              Name
            </label>
            <input
              id={`${titleId}-name`}
              type="text"
              className="crosshook-input"
              value={name}
              onChange={(e) => {
                setName(e.target.value);
                setLocalError(null);
              }}
              disabled={busy}
              autoComplete="off"
            />
            <label className="crosshook-label" htmlFor={`${titleId}-desc`}>
              Description (optional)
            </label>
            <textarea
              id={`${titleId}-desc`}
              className="crosshook-input"
              rows={3}
              value={description}
              onChange={(e) => {
                setDescription(e.target.value);
                setLocalError(null);
              }}
              disabled={busy}
            />
          </div>
          {combinedError !== null ? (
            <p className="crosshook-collection-edit-modal__warn" role="alert">
              {combinedError}
            </p>
          ) : null}
          <div className="crosshook-modal__footer-actions">
            <button type="submit" className="crosshook-button" disabled={busy}>
              {mode === 'create' ? 'Create' : 'Save'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );

  return createPortal(node, document.body);
}
