import { useCallback, useEffect, useRef, useState } from 'react';

import {
  HEALTH_BANNER_DISMISSED_SESSION_KEY,
  RENAME_TOAST_DISMISSED_SESSION_KEY,
  RENAME_TOAST_DURATION_MS,
  type RenameToast,
} from './constants';

interface UseProfilesPageNotificationsArgs {
  canRename: boolean;
  hasPendingDelete: boolean;
  profiles: string[];
  renaming: boolean;
  renameProfile: (oldName: string, newName: string) => Promise<{ hadLauncher: boolean; ok: boolean }>;
  selectedProfile: string;
  setPendingLauncherReExport: (value: boolean) => void;
}

export function useProfilesPageNotifications({
  canRename,
  hasPendingDelete,
  profiles,
  renaming,
  renameProfile,
  selectedProfile,
  setPendingLauncherReExport,
}: UseProfilesPageNotificationsArgs) {
  const [pendingRename, setPendingRename] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState('');
  const renameInputRef = useRef<HTMLInputElement>(null);
  const [renameToast, setRenameToast] = useState<RenameToast | null>(null);
  const [healthBannerDismissed, setHealthBannerDismissed] = useState(() => {
    try {
      return sessionStorage.getItem(HEALTH_BANNER_DISMISSED_SESSION_KEY) === '1';
    } catch {
      return false;
    }
  });
  const [renameToastDismissed, setRenameToastDismissed] = useState(() => {
    try {
      return sessionStorage.getItem(RENAME_TOAST_DISMISSED_SESSION_KEY) === '1';
    } catch {
      return false;
    }
  });
  const renameToastTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (pendingRename !== null) {
      renameInputRef.current?.select();
    }
  }, [pendingRename]);

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key !== 'F2') {
        return;
      }

      const target = event.target;
      if (
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        (target instanceof HTMLElement && target.isContentEditable)
      ) {
        return;
      }

      if (pendingRename !== null || hasPendingDelete) {
        return;
      }

      if (!canRename || !selectedProfile) {
        return;
      }

      event.preventDefault();
      setPendingRename(selectedProfile);
      setRenameValue(selectedProfile);
    }

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [canRename, hasPendingDelete, pendingRename, selectedProfile]);

  useEffect(() => {
    return () => {
      if (renameToastTimerRef.current !== null) {
        clearTimeout(renameToastTimerRef.current);
      }
    };
  }, []);

  const showRenameToast = useCallback((oldName: string, newName: string) => {
    if (renameToastTimerRef.current !== null) {
      clearTimeout(renameToastTimerRef.current);
    }

    setRenameToastDismissed(false);
    try {
      sessionStorage.removeItem(RENAME_TOAST_DISMISSED_SESSION_KEY);
    } catch {
      // Ignore storage errors in restricted environments.
    }

    setRenameToast({ oldName, newName });
    renameToastTimerRef.current = setTimeout(() => {
      setRenameToast(null);
      renameToastTimerRef.current = null;
    }, RENAME_TOAST_DURATION_MS);
  }, []);

  const dismissRenameToast = useCallback(() => {
    if (renameToastTimerRef.current !== null) {
      clearTimeout(renameToastTimerRef.current);
      renameToastTimerRef.current = null;
    }

    setRenameToast(null);
    setRenameToastDismissed(true);
    try {
      sessionStorage.setItem(RENAME_TOAST_DISMISSED_SESSION_KEY, '1');
    } catch {
      // Ignore storage errors in restricted environments.
    }
  }, []);

  const dismissHealthBanner = useCallback(() => {
    setHealthBannerDismissed(true);
    try {
      sessionStorage.setItem(HEALTH_BANNER_DISMISSED_SESSION_KEY, '1');
    } catch {
      // Ignore storage errors in restricted environments.
    }
  }, []);

  const undoRename = useCallback(() => {
    if (!renameToast) {
      return;
    }

    const { oldName, newName } = renameToast;
    dismissRenameToast();
    void renameProfile(newName, oldName).then(({ ok, hadLauncher }) => {
      if (!ok) {
        return;
      }

      if (hadLauncher) {
        setPendingLauncherReExport(true);
      }
    });
  }, [dismissRenameToast, renameProfile, renameToast, setPendingLauncherReExport]);

  const handleRenameConfirm = useCallback(
    (oldName: string, newName: string) => {
      setPendingRename(null);
      void renameProfile(oldName, newName).then(({ ok, hadLauncher }) => {
        if (!ok) {
          return;
        }

        showRenameToast(oldName, newName);
        if (hadLauncher) {
          setPendingLauncherReExport(true);
        }
      });
    },
    [renameProfile, setPendingLauncherReExport, showRenameToast]
  );

  const renameNameTrimmed = renameValue.trim();
  const renameIsEmpty = renameNameTrimmed.length === 0;
  const renameIsUnchanged = pendingRename !== null && renameNameTrimmed === pendingRename;
  const renameHasConflict =
    !renameIsEmpty &&
    !renameIsUnchanged &&
    profiles.some((name) => name.toLowerCase() === renameNameTrimmed.toLowerCase());
  const renameError = renameIsEmpty
    ? 'Profile name cannot be empty.'
    : renameHasConflict
      ? `A profile named '${renameNameTrimmed}' already exists.`
      : null;
  const canConfirmRename = !renameIsEmpty && !renameIsUnchanged && !renameHasConflict && !renaming;

  return {
    canConfirmRename,
    dismissHealthBanner,
    dismissRenameToast,
    handleRenameConfirm,
    healthBannerDismissed,
    pendingRename,
    renameError,
    renameInputRef,
    renameNameTrimmed,
    renameToast,
    renameToastDismissed,
    renameValue,
    setPendingRename,
    setRenameValue,
    undoRename,
  };
}
