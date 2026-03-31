import { createPortal } from 'react-dom';
import { useEffect, useRef } from 'react';

import { isSteamDeckRuntime } from '../hooks/useGamepadNav';

export type TrainerInfoModalKey = 'aurora_offline_setup' | 'wemod_offline_info';

type OfflineTrainerInfoModalProps = {
  open: boolean;
  onClose: () => void;
  modalKey: TrainerInfoModalKey | null;
};

export function OfflineTrainerInfoModal({ open, onClose, modalKey }: OfflineTrainerInfoModalProps) {
  const panelRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) {
      return;
    }
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [open, onClose]);

  if (!open || !modalKey) {
    return null;
  }

  const steamDeck = isSteamDeckRuntime();

  const title =
    modalKey === 'aurora_offline_setup'
      ? 'Aurora offline keys'
      : 'WeMod offline use';

  const node = (
    <div className="crosshook-modal" role="presentation">
      <div
        className="crosshook-modal__backdrop"
        aria-hidden="true"
        onMouseDown={(e) => {
          if (e.target === e.currentTarget) {
            onClose();
          }
        }}
      />
      <div
        ref={panelRef}
        className="crosshook-modal__surface crosshook-panel crosshook-focus-scope"
        role="dialog"
        aria-modal="true"
        aria-labelledby="crosshook-offline-trainer-info-title"
        data-crosshook-focus-root="modal"
      >
        <header className="crosshook-modal__header">
          <div className="crosshook-modal__heading-block">
            <h2 id="crosshook-offline-trainer-info-title" className="crosshook-modal__title">
              {title}
            </h2>
          </div>
          <div className="crosshook-modal__header-actions">
            <button type="button" className="crosshook-button crosshook-button--ghost crosshook-modal__close" onClick={onClose}>
              Close
            </button>
          </div>
        </header>
        <div className="crosshook-modal__body" style={{ display: 'grid', gap: 12 }}>
          {modalKey === 'aurora_offline_setup' && steamDeck ? (
            <p className="crosshook-help-text">
              <strong>Online only on Steam Deck.</strong> Aurora offline license keys are tied to Windows hardware IDs.
              Expect to run Aurora online on this device, or use a desktop Windows PC for offline key activation.
            </p>
          ) : null}
          {modalKey === 'aurora_offline_setup' && !steamDeck ? (
            <>
              <p className="crosshook-help-text">
                On desktop Linux, Aurora can work offline after you activate offline keys from Aurora on Windows (or a
                Windows VM) for the same trainer build, then copy the license bundle into your Wine prefix if Aurora
                expects it there.
              </p>
              <ol className="crosshook-help-text" style={{ margin: 0, paddingLeft: '1.2em' }}>
                <li>Open Aurora on Windows with the same trainer version and sign in if required.</li>
                <li>Use Aurora&apos;s offline / export key flow to generate device-bound keys.</li>
                <li>Copy any exported license or config files into the prefix path configured in this profile.</li>
                <li>Launch from CrossHook; hash verification helps confirm the trainer binary matches.</li>
              </ol>
            </>
          ) : null}
          {modalKey === 'wemod_offline_info' ? (
            <p className="crosshook-help-text">
              WeMod may require periodic online checks for some titles. For best offline results, prefer trainers marked
              as fully offline-capable in the catalog, and keep the trainer binary unchanged so hash checks stay green.
            </p>
          ) : null}
        </div>
      </div>
    </div>
  );

  return createPortal(node, document.body);
}
