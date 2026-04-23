import { type MouseEvent, useEffect, useRef } from 'react';
import type { LibraryOpenDetailsHandler } from './library-card-interactions';

/** Slightly above typical OS double-click interval so the second click cancels pending select. */
const DOUBLE_CLICK_GUARD_MS = 320;

/**
 * Hitbox: single-click selects (when `onSelect` is set) after a short delay; double-click opens details
 * without firing `onSelect`, since the browser emits click → click → dblclick.
 */
export function useLibraryHitboxClicks(options: {
  profileName: string;
  onOpenDetails: LibraryOpenDetailsHandler;
  onSelect?: (name: string) => void;
}) {
  const { profileName, onOpenDetails, onSelect } = options;
  const selectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const clickSeqRef = useRef(0);

  useEffect(() => {
    return () => {
      if (selectTimerRef.current) {
        clearTimeout(selectTimerRef.current);
      }
    };
  }, []);

  function handleHitboxClick() {
    if (!onSelect) {
      onOpenDetails(profileName);
      return;
    }
    clickSeqRef.current += 1;
    if (clickSeqRef.current === 1) {
      selectTimerRef.current = setTimeout(() => {
        selectTimerRef.current = null;
        if (clickSeqRef.current === 1) {
          onSelect(profileName);
        }
        clickSeqRef.current = 0;
      }, DOUBLE_CLICK_GUARD_MS);
    } else {
      if (selectTimerRef.current) {
        clearTimeout(selectTimerRef.current);
        selectTimerRef.current = null;
      }
      clickSeqRef.current = 0;
    }
  }

  function handleHitboxDoubleClick(e: MouseEvent<HTMLButtonElement>) {
    e.preventDefault();
    e.stopPropagation();
    if (selectTimerRef.current) {
      clearTimeout(selectTimerRef.current);
      selectTimerRef.current = null;
    }
    clickSeqRef.current = 0;
    onOpenDetails(profileName);
  }

  return { handleHitboxClick, handleHitboxDoubleClick };
}
