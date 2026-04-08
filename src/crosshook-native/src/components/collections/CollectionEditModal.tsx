import { createPortal } from 'react-dom';
import {
  useCallback,
  useEffect,
  useId,
  useRef,
  useState,
  type FormEvent,
  type KeyboardEvent,
  type MouseEvent,
} from 'react';

import { getFocusableElements } from '@/lib/focus-utils';

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
  const panelRef = useRef<HTMLDivElement>(null);
  const previouslyFocusedRef = useRef<HTMLElement | null>(null);
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

  useEffect(() => {
    if (!open) {
      return;
    }

    previouslyFocusedRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;

    const frame = window.requestAnimationFrame(() => {
      const panel = panelRef.current;
      if (!panel) {
        return;
      }
      const focusable = getFocusableElements(panel);
      if (focusable.length > 0) {
        focusable[0].focus({ preventScroll: true });
      }
    });

    return () => {
      window.cancelAnimationFrame(frame);
      const restoreTarget = previouslyFocusedRef.current;
      if (restoreTarget && restoreTarget.isConnected) {
        restoreTarget.focus({ preventScroll: true });
      }
      previouslyFocusedRef.current = null;
    };
  }, [open]);

  const handleKeyDown = useCallback(
    (event: KeyboardEvent<HTMLDivElement>) => {
      if (event.key === 'Escape') {
        event.stopPropagation();
        event.preventDefault();
        onClose();
        return;
      }

      if (event.key !== 'Tab') {
        return;
      }

      const panel = panelRef.current;
      if (!panel) {
        return;
      }

      const focusable = getFocusableElements(panel);
      if (focusable.length === 0) {
        event.preventDefault();
        return;
      }

      const currentIndex = focusable.indexOf(document.activeElement as HTMLElement);
      const lastIndex = focusable.length - 1;

      if (event.shiftKey) {
        if (currentIndex <= 0) {
          event.preventDefault();
          focusable[lastIndex].focus({ preventScroll: true });
        }
        return;
      }

      if (currentIndex === -1 || currentIndex === lastIndex) {
        event.preventDefault();
        focusable[0].focus({ preventScroll: true });
      }
    },
    [onClose]
  );

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
      const descNormalized = description.trim() ? description.trim() : null;
      const ok =
        mode === 'create'
          ? await onSubmitCreate(trimmed, descNormalized)
          : await onSubmitEdit(trimmed, descNormalized);
      setBusy(false);
      if (ok) {
        onClose();
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
          if (e.target === e.currentTarget) {
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
        data-crosshook-focus-root="modal"
        onKeyDown={handleKeyDown}
      >
        <header className="crosshook-modal__header">
          <div className="crosshook-modal__heading-block">
            <h2 id={titleId} className="crosshook-modal__title">
              {title}
            </h2>
          </div>
          <div className="crosshook-modal__header-actions">
            <button
              type="button"
              className="crosshook-button crosshook-button--ghost crosshook-modal__close"
              data-crosshook-modal-close
              onClick={onClose}
            >
              Close
            </button>
          </div>
        </header>
        <form className="crosshook-modal__body" onSubmit={(e) => void handleSubmit(e)}>
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
