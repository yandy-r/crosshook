import { useCallback, useEffect, useState } from 'react';
import { claimStartupAction, fetchStartupReport } from '@/lib/startupReport';

// sessionStorage: resets on window restart so the toast re-surfaces until the user explicitly dismisses it.
export const FLATPAK_MIGRATION_TOAST_SESSION_KEY = 'crosshook.flatpak.migration.toastShown';

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
    void fetchStartupReport()
      .then((report) => {
        if (!active) return;
        const migration = report.flatpak.migration;
        if (migration === null) return;
        const count = migration.imported_subtrees.length + (migration.imported_config ? 1 : 0);
        if (count === 0) return;
        try {
          if (sessionStorage.getItem(FLATPAK_MIGRATION_TOAST_SESSION_KEY) === '1') return;
        } catch {
          // sessionStorage unavailable — fall through and show the toast anyway.
        }
        if (!claimStartupAction('flatpak_migration_notice')) return;
        try {
          sessionStorage.setItem(FLATPAK_MIGRATION_TOAST_SESSION_KEY, '1');
        } catch {
          // Ignore storage errors in restricted environments.
        }
        setImportCount(count);
      })
      .catch(() => {
        // Startup report unavailable — no migration toast.
      });

    return () => {
      active = false;
    };
  }, []);

  return { importCount, dismiss };
}
