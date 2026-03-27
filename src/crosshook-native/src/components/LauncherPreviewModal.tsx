import { createPortal } from 'react-dom';
import {
  useEffect,
  useId,
  useRef,
  useState,
  type KeyboardEvent,
  type MouseEvent,
} from 'react';
import { CollapsibleSection } from './ui/CollapsibleSection';
import { copyToClipboard } from '../utils/clipboard';
import '../styles/preview.css';

/* ───────── Focus-trap helpers (mirrors LaunchPanel PreviewModal) ───────── */

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
    (el) =>
      !el.hasAttribute('disabled') &&
      el.tabIndex >= 0 &&
      el.getClientRects().length > 0,
  );
}

function focusElement(element: HTMLElement | null): boolean {
  if (!element) return false;
  element.focus({ preventScroll: true });
  return document.activeElement === element;
}

/* ───────── LauncherPreviewModal ───────── */

interface LauncherPreviewModalProps {
  scriptContent: string;
  desktopContent: string;
  displayName: string;
  onClose: () => void;
}

export function LauncherPreviewModal({
  scriptContent,
  desktopContent,
  displayName,
  onClose,
}: LauncherPreviewModalProps) {
  const portalHostRef = useRef<HTMLElement | null>(null);
  const surfaceRef = useRef<HTMLDivElement | null>(null);
  const headingRef = useRef<HTMLHeadingElement | null>(null);
  const previouslyFocusedRef = useRef<HTMLElement | null>(null);
  const bodyStyleRef = useRef('');
  const hiddenNodesRef = useRef<
    Array<{ element: HTMLElement; inert: boolean; ariaHidden: string | null }>
  >([]);
  const titleId = useId();
  const [isMounted, setIsMounted] = useState(false);
  const [copyScriptLabel, setCopyScriptLabel] = useState('Copy Script');
  const [copyDesktopLabel, setCopyDesktopLabel] = useState('Copy Desktop Entry');
  const [openSection, setOpenSection] = useState<'script' | 'desktop' | null>(null);

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

  async function handleCopyScript() {
    try {
      await copyToClipboard(scriptContent);
      setCopyScriptLabel('Copied');
      window.setTimeout(() => setCopyScriptLabel('Copy Script'), 2000);
    } catch {
      setCopyScriptLabel('Copy failed');
      window.setTimeout(() => setCopyScriptLabel('Copy Script'), 2000);
    }
  }

  async function handleCopyDesktop() {
    try {
      await copyToClipboard(desktopContent);
      setCopyDesktopLabel('Copied');
      window.setTimeout(() => setCopyDesktopLabel('Copy Desktop Entry'), 2000);
    } catch {
      setCopyDesktopLabel('Copy failed');
      window.setTimeout(() => setCopyDesktopLabel('Copy Desktop Entry'), 2000);
    }
  }

  if (!isMounted || !portalHostRef.current) return null;

  return createPortal(
    <div className="crosshook-modal" role="presentation">
      <div
        className="crosshook-modal__backdrop"
        aria-hidden="true"
        onMouseDown={handleBackdropMouseDown}
      />
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
            <div className="crosshook-heading-eyebrow">Launcher preview</div>
            <h2 ref={headingRef} id={titleId} className="crosshook-modal__title" tabIndex={-1}>
              {displayName}
            </h2>
          </div>
        </header>

        {/* Body */}
        <div className="crosshook-modal__body" style={{ gridRow: 3 }}>
          <div className="crosshook-preview-modal__sections">
            <CollapsibleSection
              title="Launcher Script"
              open={openSection === 'script'}
              onToggle={(isOpen) => {
                setOpenSection((current) => {
                  if (isOpen) return 'script';
                  return current === 'script' ? null : current;
                });
              }}
            >
              <pre className="crosshook-preview-modal__command-block">
                {scriptContent}
              </pre>
            </CollapsibleSection>

            <CollapsibleSection
              title="Desktop Entry"
              open={openSection === 'desktop'}
              onToggle={(isOpen) => {
                setOpenSection((current) => {
                  if (isOpen) return 'desktop';
                  return current === 'desktop' ? null : current;
                });
              }}
            >
              <pre className="crosshook-preview-modal__command-block">
                {desktopContent}
              </pre>
            </CollapsibleSection>
          </div>
        </div>

        {/* Footer */}
        <footer className="crosshook-modal__footer" style={{ gridRow: 4 }}>
          <span />
          <div className="crosshook-modal__footer-actions">
            <button
              type="button"
              className="crosshook-button crosshook-button--ghost"
              onClick={() => void handleCopyScript()}
            >
              {copyScriptLabel}
            </button>
            <button
              type="button"
              className="crosshook-button crosshook-button--ghost"
              onClick={() => void handleCopyDesktop()}
            >
              {copyDesktopLabel}
            </button>
            <button
              type="button"
              className="crosshook-button crosshook-button--ghost"
              onClick={onClose}
            >
              Close
            </button>
          </div>
        </footer>
      </div>
    </div>,
    portalHostRef.current,
  );
}

export default LauncherPreviewModal;
