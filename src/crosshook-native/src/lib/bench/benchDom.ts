/** DOM helpers shared by the bench scenarios and probes. They never add nodes or styles. */

export function focusTarget(element: HTMLElement): void {
  element.focus({ preventScroll: true });
}

/**
 * Radix Tabs (automatic activation) switches route on `focus`, which would fire
 * the change before `armed`. Swallow that one focusin so only the runner's key
 * triggers the switch. Capture on window runs before React's root listener.
 */
export function focusWithoutActivation(element: HTMLElement): void {
  const swallow = (event: Event) => event.stopImmediatePropagation();
  window.addEventListener('focusin', swallow, true);
  try {
    focusTarget(element);
  } finally {
    window.removeEventListener('focusin', swallow, true);
  }
}

/** Radix tab triggers/content carry the tab value as the id suffix (`<base>-trigger-<value>`). */
export function tabTrigger(value: string): HTMLElement | null {
  return document.querySelector<HTMLElement>(`[role="tab"][id$="-trigger-${value}"]`);
}

/** Radix tabs activate on mousedown (not click); a click covers plain buttons. */
export function activate(element: Element): void {
  element.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, cancelable: true, button: 0 }));
  if (element instanceof HTMLElement) {
    element.click();
  }
}

/**
 * Resolves with the first element matching `selector`, or null after `timeoutMs` or when
 * `signal` aborts (an already-aborted signal resolves null at once, even if the element exists).
 */
export function waitForElement(selector: string, timeoutMs: number, signal?: AbortSignal): Promise<Element | null> {
  if (signal?.aborted) {
    return Promise.resolve(null);
  }
  const existing = document.querySelector(selector);
  if (existing) {
    return Promise.resolve(existing);
  }
  return new Promise((resolve) => {
    const finish = (found: Element | null) => {
      observer.disconnect();
      clearTimeout(timer);
      signal?.removeEventListener('abort', onAbort);
      resolve(found);
    };
    const onAbort = () => finish(null);
    const observer = new MutationObserver(() => {
      const found = document.querySelector(selector);
      if (found) {
        finish(found);
      }
    });
    const timer = setTimeout(() => finish(null), timeoutMs);
    signal?.addEventListener('abort', onAbort, { once: true });
    observer.observe(document.body, { childList: true, subtree: true });
  });
}
