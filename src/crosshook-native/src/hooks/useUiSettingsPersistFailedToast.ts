import { useCallback, useEffect, useRef, useState } from 'react';
import { subscribeEvent } from '@/lib/events';

export const UI_SETTINGS_PERSIST_FAILED_EVENT = 'ui-settings-persist-failed';

export interface UiSettingsPersistFailedPayload {
  /** Sanitized `AppError` text from the native side; shown only via `title`. */
  message: string;
}

export const UI_SETTINGS_PERSIST_FAILED_MESSAGE = "Couldn't save display preferences. Changes apply until restart.";

export interface UseUiSettingsPersistFailedToastResult {
  /** Sanitized backend message when the toast should be shown; `null` hides it. */
  detailMessage: string | null;
  dismiss: () => void;
}

/**
 * Shows at most one error toast per session when `[ui]` settings fail to
 * persist. Runtime-only state: the first event shows the toast and any later
 * events in the same session are ignored; a full window restart resets it.
 */
export function useUiSettingsPersistFailedToast(): UseUiSettingsPersistFailedToastResult {
  const [detailMessage, setDetailMessage] = useState<string | null>(null);
  /** Session-scoped latch: once shown, later events (even after dismiss) are ignored. */
  const shownRef = useRef(false);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    void subscribeEvent<UiSettingsPersistFailedPayload>(UI_SETTINGS_PERSIST_FAILED_EVENT, (event) => {
      if (!active || shownRef.current) return;
      shownRef.current = true;
      setDetailMessage(event.payload.message);
    }).then((unsub) => {
      if (active) {
        unlisten = unsub;
      } else {
        unsub();
      }
    });
    return () => {
      active = false;
      unlisten?.();
    };
  }, []);

  const dismiss = useCallback(() => {
    setDetailMessage(null);
  }, []);

  return { detailMessage, dismiss };
}
