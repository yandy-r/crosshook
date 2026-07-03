import { useCallback, useEffect, useMemo, useRef } from 'react';

export interface DebouncedScheduler {
  /** Replace any pending callback and run `callback` after the delay. */
  schedule: (callback: () => void) => void;
  /** Drop the pending callback, if any. */
  cancel: () => void;
}

/**
 * Shared debounce timer: one pending callback at a time, cleared on unmount.
 */
export function useDebounce(delayMs: number): DebouncedScheduler {
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const cancel = useCallback(() => {
    if (timerRef.current !== null) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const schedule = useCallback(
    (callback: () => void) => {
      cancel();
      timerRef.current = setTimeout(() => {
        timerRef.current = null;
        callback();
      }, delayMs);
    },
    [cancel, delayMs]
  );

  useEffect(() => cancel, [cancel]);

  return useMemo(() => ({ schedule, cancel }), [schedule, cancel]);
}
