// Roving-tabindex primitive: the container is a single tab stop; ArrowUp/ArrowDown
// move between items (wrapping), Home/End jump. Reference adoption is the sidebar
// Collections block; the palette list and discovery results are follow-up adopters
// (future issues).
import { useEffect, useRef, useState } from 'react';
import { isRovingCandidate } from '@/lib/focus-utils';

export interface RovingTabindexOptions {
  /** e.g. '[data-roving-item]' */
  itemSelector: string;
}

/**
 * Returns a callback ref to attach to the container element. Using a callback
 * ref (React state) instead of a ref object means the listeners attach whenever
 * the container mounts, including containers rendered conditionally after the
 * first render.
 */
export function useRovingTabindex({ itemSelector }: RovingTabindexOptions): (node: HTMLElement | null) => void {
  const [container, setContainer] = useState<HTMLElement | null>(null);
  const activeIndexRef = useRef(0);

  useEffect(() => {
    if (!container) return;

    // Single querySelectorAll pass: re-reads the item list, clamps the active
    // index, and parks every non-active (and non-candidate: disabled,
    // aria-hidden, invisible) element at tabIndex -1 so the group stays a
    // single tab stop.
    const normalize = (): HTMLElement[] => {
      const all = Array.from(container.querySelectorAll<HTMLElement>(itemSelector));
      const items = all.filter(isRovingCandidate);
      if (activeIndexRef.current >= items.length) {
        activeIndexRef.current = 0;
      }
      const active = items[activeIndexRef.current] ?? null;
      for (const element of all) {
        element.tabIndex = element === active ? 0 : -1;
      }
      return items;
    };

    const focusAt = (items: HTMLElement[], index: number): void => {
      if (items.length === 0) return;
      const clamped = Math.max(0, Math.min(index, items.length - 1));
      activeIndexRef.current = clamped;
      const active = items[clamped];
      for (const element of items) {
        element.tabIndex = element === active ? 0 : -1;
      }
      active.focus();
    };

    normalize();

    const onKeyDown = (event: KeyboardEvent): void => {
      const items = normalize();
      switch (event.key) {
        case 'ArrowDown':
          event.preventDefault();
          if (items.length > 0) focusAt(items, (activeIndexRef.current + 1) % items.length);
          return;
        case 'ArrowUp':
          event.preventDefault();
          if (items.length > 0) focusAt(items, (activeIndexRef.current - 1 + items.length) % items.length);
          return;
        case 'Home':
          event.preventDefault();
          focusAt(items, 0);
          return;
        case 'End':
          event.preventDefault();
          focusAt(items, items.length - 1);
          return;
        default:
      }
    };

    const onFocusIn = (event: FocusEvent): void => {
      const target = event.target instanceof HTMLElement ? event.target.closest<HTMLElement>(itemSelector) : null;
      if (!target) return;
      const items = normalize();
      const index = items.indexOf(target);
      if (index < 0) return;
      activeIndexRef.current = index;
      for (const element of items) {
        element.tabIndex = element === target ? 0 : -1;
      }
    };

    const observer = new MutationObserver(() => {
      normalize();
    });

    container.addEventListener('keydown', onKeyDown);
    container.addEventListener('focusin', onFocusIn);
    observer.observe(container, { childList: true, subtree: true });

    return () => {
      container.removeEventListener('keydown', onKeyDown);
      container.removeEventListener('focusin', onFocusIn);
      observer.disconnect();
    };
  }, [container, itemSelector]);

  return setContainer;
}
