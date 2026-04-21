import { useCallback, useEffect, useState } from 'react';
import { subscribeEvent } from '@/lib/events';

// sessionStorage: resets on window restart so the toast re-surfaces until the user explicitly dismisses it.
export const FLATPAK_MIGRATION_TOAST_SESSION_KEY = 'crosshook.flatpak.migration.toastShown';
const EVENT_NAME = 'flatpak-migration-complete';

export type FlatpakMigrationCompletePayload = {
  imported_config: boolean;
  imported_subtrees: string[];
  skipped_subtrees: string[];
};

export interface UseFlatpakMigrationToastResult {
  /** Number of items imported; null means no toast should be shown. */
  importCount: number | null;
  dismiss: () => void;
}

export function useFlatpakMigrationToast(): UseFlatpakMigrationToastResult {
  const [importCount, setImportCount] = useState<number | null>(null);

  const dismiss = useCallback(() => {
    setImportCount(null);
    try {
      sessionStorage.setItem(FLATPAK_MIGRATION_TOAST_SESSION_KEY, '1');
    } catch {
      // Ignore storage errors in restricted environments.
    }
  }, []);

  useEffect(() => {
    let active = true;
    const unlistenPromise = subscribeEvent<FlatpakMigrationCompletePayload>(EVENT_NAME, (event) => {
      if (!active) return;
      const { imported_config, imported_subtrees } = event.payload;
      const count = imported_subtrees.length + (imported_config ? 1 : 0);
      if (count === 0) return;
      try {
        if (sessionStorage.getItem(FLATPAK_MIGRATION_TOAST_SESSION_KEY) === '1') return;
        sessionStorage.setItem(FLATPAK_MIGRATION_TOAST_SESSION_KEY, '1');
      } catch {
        // sessionStorage unavailable — fall through and show the toast anyway.
      }
      setImportCount(count);
    });

    return () => {
      active = false;
      void unlistenPromise
        .then((unlisten) => {
          unlisten();
        })
        .catch(() => {
          // subscribeEvent may reject; ignore during teardown.
        });
    };
  }, []);

  return { importCount, dismiss };
}
