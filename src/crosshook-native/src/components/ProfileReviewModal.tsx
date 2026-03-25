import { createPortal } from 'react-dom';
import {
  useEffect,
  useId,
  useRef,
  useState,
  type CSSProperties,
  type MouseEvent,
  type ReactNode,
  type RefObject,
  type KeyboardEvent,
} from 'react';

export type ProfileReviewModalStatusTone = 'neutral' | 'success' | 'warning' | 'danger';

export interface ProfileReviewModalConfirmation {
  title: string;
  body: ReactNode;
  confirmLabel: string;
  cancelLabel: string;
  tone?: ProfileReviewModalStatusTone;
  onConfirm: () => void;
  onCancel: () => void;
}

export interface ProfileReviewModalProps {
  open: boolean;
  title: string;
  statusLabel: string;
  profileName: string;
  executablePath: string;
  prefixPath: string;
  helperLogPath: string;
  children: ReactNode;
  footer?: ReactNode;
  description?: string;
  onClose: () => void;
  allowBackdropDismiss?: boolean;
  closeLabel?: string;
  initialFocusRef?: RefObject<HTMLElement | null>;
  statusTone?: ProfileReviewModalStatusTone;
  className?: string;
  confirmation?: ProfileReviewModalConfirmation | null;
}

const FOCUSABLE_SELECTOR = [
  'a[href]',
  'button:not([disabled])',
  'input:not([disabled]):not([type="hidden"])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
  '[contenteditable="true"]',
].join(', ');

function getFocusableElements(container: HTMLElement) {
  return Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(
    (element) =>
      !element.hasAttribute('disabled') &&
      element.tabIndex >= 0 &&
      element.getClientRects().length > 0,
  );
}

function focusElement(element: HTMLElement | null) {
  if (!element) {
    return false;
  }

  element.focus({ preventScroll: true });
  return document.activeElement === element;
}

function formatSummaryValue(value: string) {
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : 'Not set';
}

function resolveStatusToneClass(tone: ProfileReviewModalStatusTone) {
  switch (tone) {
    case 'success':
      return 'crosshook-modal__status-chip--success';
    case 'warning':
      return 'crosshook-modal__status-chip--warning';
    case 'danger':
      return 'crosshook-modal__status-chip--danger';
    default:
      return 'crosshook-modal__status-chip--neutral';
  }
}

const confirmationBackdropStyle: CSSProperties = {
  position: 'absolute',
  inset: 0,
  zIndex: 5,
  display: 'grid',
  placeItems: 'center',
  padding: 24,
  background: 'rgba(3, 8, 20, 0.76)',
  backdropFilter: 'blur(10px)',
};

const confirmationCardStyle: CSSProperties = {
  width: 'min(560px, 100%)',
  display: 'grid',
  gap: 16,
  padding: 24,
  borderRadius: 20,
  border: '1px solid rgba(96, 165, 250, 0.24)',
  background:
    'radial-gradient(circle at top right, rgba(0, 120, 212, 0.12), transparent 26%), linear-gradient(180deg, rgba(15, 23, 42, 0.98), rgba(8, 12, 24, 0.98))',
  boxShadow: '0 24px 60px rgba(0, 0, 0, 0.46)',
};

export function ProfileReviewModal({
  open,
  title,
  statusLabel,
  profileName,
  executablePath,
  prefixPath,
  helperLogPath,
  children,
  footer,
  description,
  onClose,
  allowBackdropDismiss = true,
  closeLabel = 'Close review dialog',
  initialFocusRef,
  statusTone = 'neutral',
  className,
  confirmation = null,
}: ProfileReviewModalProps) {
  const portalHostRef = useRef<HTMLElement | null>(null);
  const surfaceRef = useRef<HTMLDivElement | null>(null);
  const confirmationRef = useRef<HTMLDivElement | null>(null);
  const closeButtonRef = useRef<HTMLButtonElement | null>(null);
  const headingRef = useRef<HTMLHeadingElement | null>(null);
  const previouslyFocusedRef = useRef<HTMLElement | null>(null);
  const bodyStyleRef = useRef<string>('');
  const hiddenNodesRef = useRef<
    Array<{ element: HTMLElement; inert: boolean; ariaHidden: string | null }>
  >([]);
  const titleId = useId();
  const descriptionId = useId();
  const [isMounted, setIsMounted] = useState(false);

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

  useEffect(() => {
    if (!open || typeof document === 'undefined') {
      return;
    }

    const { body } = document;
    const portalHost = portalHostRef.current;

    if (!portalHost) {
      return;
    }

    previouslyFocusedRef.current =
      document.activeElement instanceof HTMLElement ? document.activeElement : null;

    bodyStyleRef.current = body.style.overflow;
    body.style.overflow = 'hidden';
    body.classList.add('crosshook-modal-open');

    hiddenNodesRef.current = Array.from(body.children)
      .filter(
        (child): child is HTMLElement =>
          child instanceof HTMLElement && child !== portalHost,
      )
      .map((element) => {
        const inertState = (element as HTMLElement & { inert?: boolean }).inert ?? false;
        const ariaHidden = element.getAttribute('aria-hidden');
        (element as HTMLElement & { inert?: boolean }).inert = true;
        element.setAttribute('aria-hidden', 'true');
        return { element, inert: inertState, ariaHidden };
      });

    const focusTarget =
      initialFocusRef?.current ?? headingRef.current ?? closeButtonRef.current ?? null;

    const frame = window.requestAnimationFrame(() => {
      if (focusElement(focusTarget)) {
        return;
      }

      const focusable = surfaceRef.current ? getFocusableElements(surfaceRef.current) : [];
      if (focusable.length > 0) {
        focusElement(focusable[0]);
      }
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
  }, [initialFocusRef, open]);

  function handleKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === 'Escape') {
      event.stopPropagation();
      event.preventDefault();
      if (confirmation) {
        confirmation.onCancel();
      } else {
        onClose();
      }
      return;
    }

    if (event.key !== 'Tab') {
      return;
    }

    const focusContainer = confirmation ? confirmationRef.current : surfaceRef.current;
    if (!focusContainer) {
      return;
    }

    const focusable = getFocusableElements(focusContainer);
    if (focusable.length === 0) {
      event.preventDefault();
      focusElement(headingRef.current ?? closeButtonRef.current);
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
    if (event.target !== event.currentTarget) {
      return;
    }

    if (confirmation) {
      confirmation.onCancel();
      return;
    }

    if (!allowBackdropDismiss) {
      return;
    }

    onClose();
  }

  function handleRequestClose() {
    if (confirmation) {
      confirmation.onCancel();
      return;
    }

    onClose();
  }

  if (!open || !isMounted || !portalHostRef.current) {
    return null;
  }

  return createPortal(
    <div className="crosshook-modal" role="presentation">
      <div
        className="crosshook-modal__backdrop"
        aria-hidden="true"
        onMouseDown={handleBackdropMouseDown}
      />
      <div
        ref={surfaceRef}
        className={[
          'crosshook-modal__surface',
          'crosshook-panel',
          'crosshook-focus-scope',
          className,
        ]
          .filter(Boolean)
          .join(' ')}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={description ? descriptionId : undefined}
        data-crosshook-focus-root={confirmation ? undefined : 'modal'}
        onKeyDown={handleKeyDown}
      >
        <header className="crosshook-modal__header">
          <div className="crosshook-modal__heading-block">
            <div className="crosshook-heading-eyebrow">Profile review</div>
            <h2 ref={headingRef} id={titleId} className="crosshook-modal__title" tabIndex={-1}>
              {title}
            </h2>
            {description ? (
              <p id={descriptionId} className="crosshook-modal__description">
                {description}
              </p>
            ) : null}
          </div>

          <div className="crosshook-modal__header-actions">
            <span
              className={[
                'crosshook-modal__status-chip',
                resolveStatusToneClass(statusTone),
              ].join(' ')}
            >
              {statusLabel}
            </span>
            <button
              ref={closeButtonRef}
              type="button"
              className="crosshook-button crosshook-button--ghost crosshook-modal__close"
              onClick={handleRequestClose}
              aria-label={closeLabel}
              data-crosshook-modal-close
            >
              Close
            </button>
          </div>
        </header>

        <section className="crosshook-modal__summary" aria-label="Review summary">
          <div className="crosshook-modal__summary-item">
            <div className="crosshook-modal__summary-label">Install status</div>
            <div className="crosshook-modal__summary-value">{formatSummaryValue(statusLabel)}</div>
          </div>
          <div className="crosshook-modal__summary-item">
            <div className="crosshook-modal__summary-label">Profile name</div>
            <div className="crosshook-modal__summary-value">{formatSummaryValue(profileName)}</div>
          </div>
          <div className="crosshook-modal__summary-item">
            <div className="crosshook-modal__summary-label">Executable</div>
            <div className="crosshook-modal__summary-value crosshook-modal__summary-value--mono">
              {formatSummaryValue(executablePath)}
            </div>
          </div>
          <div className="crosshook-modal__summary-item">
            <div className="crosshook-modal__summary-label">Prefix</div>
            <div className="crosshook-modal__summary-value crosshook-modal__summary-value--mono">
              {formatSummaryValue(prefixPath)}
            </div>
          </div>
          <div className="crosshook-modal__summary-item">
            <div className="crosshook-modal__summary-label">Helper log</div>
            <div className="crosshook-modal__summary-value crosshook-modal__summary-value--mono">
              {formatSummaryValue(helperLogPath)}
            </div>
          </div>
        </section>

        <div className="crosshook-modal__body">{children}</div>

        <footer className="crosshook-modal__footer">
          <div className="crosshook-modal__footer-copy">
            Review the generated profile before saving it to disk. Saving switches you to the Profile tab for
            further edits.
          </div>
          {footer ? <div className="crosshook-modal__footer-actions">{footer}</div> : null}
        </footer>

        {confirmation ? (
          <div
            className="crosshook-modal__confirmation"
            style={confirmationBackdropStyle}
            role="alertdialog"
            aria-modal="true"
            aria-labelledby={`${titleId}-confirmation`}
            aria-describedby={`${descriptionId}-confirmation`}
            onMouseDown={handleBackdropMouseDown}
          >
            <div
              ref={confirmationRef}
              className="crosshook-panel"
              style={confirmationCardStyle}
              data-crosshook-focus-root="modal"
            >
              <div className="crosshook-modal__heading-block">
                <div className="crosshook-heading-eyebrow">Confirmation required</div>
                <h3 id={`${titleId}-confirmation`} className="crosshook-modal__title" style={{ fontSize: '1.35rem' }}>
                  {confirmation.title}
                </h3>
                <p id={`${descriptionId}-confirmation`} className="crosshook-modal__description">
                  {confirmation.body}
                </p>
              </div>

              <div className="crosshook-modal__header-actions" style={{ justifyContent: 'flex-end' }}>
                <button
                  type="button"
                  className="crosshook-button crosshook-button--secondary"
                  onClick={confirmation.onCancel}
                  data-crosshook-modal-close
                >
                  {confirmation.cancelLabel}
                </button>
                <button type="button" className="crosshook-button" onClick={confirmation.onConfirm}>
                  {confirmation.confirmLabel}
                </button>
              </div>
            </div>
          </div>
        ) : null}
      </div>
    </div>,
    portalHostRef.current,
  );
}

export default ProfileReviewModal;
