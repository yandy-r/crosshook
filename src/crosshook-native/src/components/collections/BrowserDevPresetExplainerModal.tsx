import { createPortal } from 'react-dom';
import { useCallback, useEffect, useId, useRef, useState, type MouseEvent } from 'react';

import { useFocusTrap } from '@/hooks/useFocusTrap';

import './BrowserDevPresetExplainerModal.css';

export type BrowserDevPresetExplainerMode = 'import' | 'export';

export interface BrowserDevPresetExplainerModalProps {
  mode: BrowserDevPresetExplainerMode;
  open: boolean;
  onClose: () => void;
  onContinue: () => void | Promise<void>;
}

const COPY: Record<BrowserDevPresetExplainerMode, { title: string; paragraphs: string[]; continueLabel: string }> = {
  import: {
    title: 'Import Preset',
    paragraphs: [
      'In the CrossHook desktop app, **Import Preset** opens a file picker so you can choose a `.crosshook-collection.toml` file from disk. CrossHook then shows a preview where you can adjust the name and resolve profile matches before creating the collection.',
      'Browser dev mode does not use the real file dialog. **Continue** runs the same preview step against mock data so you can iterate on the UI.',
    ],
    continueLabel: 'Continue',
  },
  export: {
    title: 'Export Preset',
    paragraphs: [
      'In the CrossHook desktop app, **Export Preset** opens a save dialog so you can choose where to write the preset file.',
      'Browser dev mode does not use the real save dialog. **Continue** performs a mock export through the dev IPC layer so you can verify the flow.',
    ],
    continueLabel: 'Continue',
  },
};

/**
 * Renders a paragraph with a minimal `**bold**` markdown-like parser.
 *
 * SECURITY: This helper MUST only receive trusted, static strings (the `COPY`
 * constant in this file). It does NOT escape or sanitize input beyond React's
 * default text escaping, and the pattern splitter has no notion of nested or
 * malformed markers. Never call this with user-supplied text — if that need
 * arises, replace it with a proper markdown renderer.
 */
function renderParagraph(text: string) {
  const parts = text.split(/\*\*(.+?)\*\*/g);
  return <p>{parts.map((part, i) => (i % 2 === 1 ? <strong key={i}>{part}</strong> : <span key={i}>{part}</span>))}</p>;
}

export function BrowserDevPresetExplainerModal({
  mode,
  open,
  onClose,
  onContinue,
}: BrowserDevPresetExplainerModalProps) {
  const titleId = useId();
  const descriptionId = useId();
  const panelRef = useRef<HTMLDivElement>(null);
  const headingRef = useRef<HTMLHeadingElement>(null);
  const portalHostRef = useRef<HTMLElement | null>(null);
  const [isMounted, setIsMounted] = useState(false);
  const [busy, setBusy] = useState(false);

  const { title, paragraphs, continueLabel } = COPY[mode];

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
    if (!busy) onClose();
  }, [busy, onClose]);

  const { handleKeyDown } = useFocusTrap({
    open,
    panelRef,
    onClose: guardedOnClose,
    initialFocusRef: headingRef,
  });

  const handleContinue = useCallback(async () => {
    setBusy(true);
    try {
      await onContinue();
    } catch (error) {
      console.error('BrowserDevPresetExplainerModal: onContinue failed', error);
    } finally {
      setBusy(false);
    }
  }, [onContinue]);

  if (!open || !isMounted || !portalHostRef.current) {
    return null;
  }

  const node = (
    <div className="crosshook-modal crosshook-browser-dev-preset-explainer" role="presentation">
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
        className="crosshook-modal__surface crosshook-panel crosshook-focus-scope"
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
        <div id={descriptionId} className="crosshook-browser-dev-preset-explainer__body">
          {paragraphs.map((p, i) => (
            <div key={i}>{renderParagraph(p)}</div>
          ))}
        </div>
        <footer className="crosshook-modal__footer">
          <div className="crosshook-modal__footer-actions">
            <button
              type="button"
              className="crosshook-button crosshook-button--ghost"
              data-crosshook-modal-close
              disabled={busy}
              onClick={onClose}
            >
              Cancel
            </button>
            <button type="button" className="crosshook-button" disabled={busy} onClick={() => void handleContinue()}>
              {continueLabel}
            </button>
          </div>
        </footer>
      </div>
    </div>
  );

  return createPortal(node, portalHostRef.current);
}
