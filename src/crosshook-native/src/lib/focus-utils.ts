/** Selectors for elements that participate in modal/tab focus traps. */
export const FOCUSABLE_SELECTOR = [
  'a[href]',
  'button:not([disabled])',
  'input:not([disabled]):not([type="hidden"])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
  '[contenteditable="true"]',
].join(', ');

export function getFocusableElements(container: HTMLElement): HTMLElement[] {
  return Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(
    (el) => !el.hasAttribute('disabled') && el.tabIndex >= 0 && el.getClientRects().length > 0
  );
}

function isVisible(element: HTMLElement): boolean {
  const style = window.getComputedStyle(element);
  return (
    style.display !== 'none' &&
    style.visibility !== 'hidden' &&
    style.opacity !== '0' &&
    element.getClientRects().length > 0
  );
}

/** Enabled, not aria-hidden, visible — ignores tabindex (roving items are tabindex=-1 by design). */
export function isRovingCandidate(element: HTMLElement): boolean {
  if (element.hasAttribute('disabled') || element.getAttribute('aria-hidden') === 'true') {
    return false;
  }

  return isVisible(element);
}

export function isFocusable(element: HTMLElement): boolean {
  return isRovingCandidate(element) && element.getAttribute('tabindex') !== '-1';
}
