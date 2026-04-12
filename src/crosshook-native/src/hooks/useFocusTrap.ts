import { type KeyboardEvent, type RefObject, useEffect, useRef } from 'react';

import { getFocusableElements } from '@/lib/focus-utils';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/** Options accepted by {@link useFocusTrap}. */
export interface UseFocusTrapOptions {
  /** Whether the trap is active (e.g. a modal is open). */
  open: boolean;

  /**
   * Ref to the panel/surface element that contains the focusable children.
   * Tab cycling and initial-focus lookup are scoped to this container.
   */
  panelRef: RefObject<HTMLElement | null>;

  /** Called when the user presses Escape while the trap is active. */
  onClose: () => void;

  /**
   * Optional ref to the element that should receive initial focus when the
   * trap activates (e.g. a heading with `tabIndex={-1}`). Falls back to the
   * first focusable descendant of `panelRef`.
   */
  initialFocusRef?: RefObject<HTMLElement | null>;
}

/** Value returned by {@link useFocusTrap}. */
export interface UseFocusTrapReturn {
  /**
   * Keyboard handler that traps Tab focus cycling and handles Escape.
   * Attach as `onKeyDown` on the trap container element.
   */
  handleKeyDown: (event: KeyboardEvent<HTMLElement>) => void;
}

// ---------------------------------------------------------------------------
// Global modal stack (nested / overlapping traps)
// ---------------------------------------------------------------------------

/** Depth of open `useFocusTrap` instances that locked the body. */
let modalBodyLockDepth = 0;
let savedBodyOverflow = '';

interface InertRegistryEntry {
  count: number;
  inert: boolean;
  ariaHidden: string | null;
}

/** Per-element ref-count so out-of-order cleanups do not restore shared DOM too early. */
const modalInertRegistry = new Map<HTMLElement, InertRegistryEntry>();

function registerInertElement(element: HTMLElement): void {
  const existing = modalInertRegistry.get(element);
  if (existing) {
    existing.count += 1;
    return;
  }
  const inert = (element as HTMLElement & { inert?: boolean }).inert ?? false;
  const ariaHidden = element.getAttribute('aria-hidden');
  (element as HTMLElement & { inert?: boolean }).inert = true;
  element.setAttribute('aria-hidden', 'true');
  modalInertRegistry.set(element, { count: 1, inert, ariaHidden });
}

function unregisterInertElement(element: HTMLElement): void {
  const entry = modalInertRegistry.get(element);
  if (!entry) return;
  entry.count -= 1;
  if (entry.count > 0) return;
  (element as HTMLElement & { inert?: boolean }).inert = entry.inert;
  if (entry.ariaHidden === null) {
    element.removeAttribute('aria-hidden');
  } else {
    element.setAttribute('aria-hidden', entry.ariaHidden);
  }
  modalInertRegistry.delete(element);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Attempt to focus an element, returning `true` when focus actually moved.
 * Uses `preventScroll` to avoid layout shifting when the trap activates.
 */
function focusElement(el: HTMLElement | null): boolean {
  if (!el) return false;
  el.focus({ preventScroll: true });
  return document.activeElement === el;
}

/**
 * Walk up from `el` to find the direct child of `document.body` that
 * contains it (i.e. the portal host).
 */
function findPortalHost(el: HTMLElement): HTMLElement | null {
  let current: HTMLElement | null = el;
  while (current && current.parentElement !== document.body) {
    current = current.parentElement;
  }
  return current;
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

/**
 * Focus-trap hook extracted from the modal accessibility pattern used across
 * CrossHook modals.
 *
 * When `open` is `true` the hook:
 *
 * 1. Saves `document.activeElement` so focus can be restored on close.
 * 2. Sets `body.style.overflow = 'hidden'` and adds the
 *    `crosshook-modal-open` class to prevent background scroll (ref-counted when
 *    multiple traps are active).
 * 3. Marks sibling elements of the portal host as `inert` / `aria-hidden`
 *    so screen readers cannot escape the modal (ref-counted per element).
 * 4. Moves focus to `initialFocusRef` (or the first focusable descendant)
 *    via `requestAnimationFrame`.
 * 5. Returns a `handleKeyDown` that traps Tab cycling within the panel and
 *    calls `onClose` on Escape.
 *
 * All side-effects are reversed on cleanup.
 *
 * @example
 * ```tsx
 * const panelRef = useRef<HTMLDivElement>(null);
 * const headingRef = useRef<HTMLHeadingElement>(null);
 * const { handleKeyDown } = useFocusTrap({
 *   open,
 *   panelRef,
 *   onClose,
 *   initialFocusRef: headingRef,
 * });
 *
 * return (
 *   <div ref={panelRef} onKeyDown={handleKeyDown} role="dialog">
 *     <h2 ref={headingRef} tabIndex={-1}>Title</h2>
 *     ...
 *   </div>
 * );
 * ```
 */
export function useFocusTrap({ open, panelRef, onClose, initialFocusRef }: UseFocusTrapOptions): UseFocusTrapReturn {
  const previouslyFocusedRef = useRef<HTMLElement | null>(null);
  /** Elements this instance registered with {@link modalInertRegistry}. */
  const touchedInertRef = useRef<HTMLElement[]>([]);
  /** Suppresses the deferred focus-restore microtask after cleanup runs. */
  const microtaskSuppressRef = useRef<boolean>(false);

  useEffect(() => {
    if (!open || typeof document === 'undefined') return;

    microtaskSuppressRef.current = false;

    const { body } = document;
    const panel = panelRef.current;
    // Find the portal host — walk up from panel to find the direct child of body
    const portalHost = panel ? findPortalHost(panel) : null;
    if (!portalHost) return;

    // Save current focus
    previouslyFocusedRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;

    modalBodyLockDepth += 1;
    if (modalBodyLockDepth === 1) {
      savedBodyOverflow = body.style.overflow;
      body.style.overflow = 'hidden';
      body.classList.add('crosshook-modal-open');
    }

    const touched: HTMLElement[] = [];
    for (const child of Array.from(body.children)) {
      if (!(child instanceof HTMLElement) || child === portalHost) continue;
      registerInertElement(child);
      touched.push(child);
    }
    touchedInertRef.current = touched;

    // Focus initial target
    const focusTarget = initialFocusRef?.current ?? null;
    const frame = window.requestAnimationFrame(() => {
      if (focusElement(focusTarget)) return;
      const focusable = panel ? getFocusableElements(panel) : [];
      if (focusable.length > 0) {
        focusElement(focusable[0]);
      }
    });

    return () => {
      microtaskSuppressRef.current = true;
      window.cancelAnimationFrame(frame);
      for (const el of touchedInertRef.current) {
        unregisterInertElement(el);
      }
      touchedInertRef.current = [];

      modalBodyLockDepth = Math.max(0, modalBodyLockDepth - 1);
      if (modalBodyLockDepth === 0) {
        body.style.overflow = savedBodyOverflow;
        body.classList.remove('crosshook-modal-open');
      }

      const restoreTarget = previouslyFocusedRef.current;
      previouslyFocusedRef.current = null;
      // Defer restore so another modal mounted in the same React commit can render
      // first; skip if a modal is still open so focus stays with the new trap.
      queueMicrotask(() => {
        if (microtaskSuppressRef.current) {
          return;
        }
        if (typeof document === 'undefined') {
          return;
        }
        if (document.querySelector('[data-crosshook-focus-root="modal"]')) {
          return;
        }
        if (restoreTarget?.isConnected) {
          focusElement(restoreTarget);
        }
      });
    };
  }, [open, panelRef, initialFocusRef]);

  function handleKeyDown(event: KeyboardEvent<HTMLElement>) {
    if (event.key === 'Escape') {
      event.stopPropagation();
      event.preventDefault();
      onClose();
      return;
    }
    if (event.key !== 'Tab') return;
    const container = panelRef.current;
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

  return { handleKeyDown };
}
