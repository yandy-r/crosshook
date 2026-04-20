import type { MutableRefObject } from 'react';

import {
  CONTENT_FALLBACK_SELECTOR,
  FOCUS_ZONE_ATTRIBUTE,
  FOCUSABLE_SELECTOR,
  MODAL_FOCUS_ROOT_SELECTOR,
  SIDEBAR_FALLBACK_SELECTOR,
} from './constants';
import type { FocusZone } from './types';

export function isVisible(element: HTMLElement): boolean {
  const style = window.getComputedStyle(element);
  return (
    style.display !== 'none' &&
    style.visibility !== 'hidden' &&
    style.opacity !== '0' &&
    element.getClientRects().length > 0
  );
}

export function isFocusable(element: HTMLElement): boolean {
  if (element.hasAttribute('disabled') || element.getAttribute('aria-hidden') === 'true') {
    return false;
  }

  const tabIndex = element.getAttribute('tabindex');
  if (tabIndex === '-1') {
    return false;
  }

  return isVisible(element);
}

export function getRootElement(rootRef: MutableRefObject<HTMLElement | null>): HTMLElement | null {
  return rootRef.current;
}

export function getNavigationRoot(rootRef: MutableRefObject<HTMLElement | null>): HTMLElement | null {
  const modalRoots = document.querySelectorAll<HTMLElement>(MODAL_FOCUS_ROOT_SELECTOR);
  return modalRoots.item(modalRoots.length - 1) ?? getRootElement(rootRef);
}

export function isModalNavigationRoot(root: HTMLElement | null): boolean {
  return root?.matches(MODAL_FOCUS_ROOT_SELECTOR) ?? false;
}

export function getFocusZoneRoot(root: HTMLElement | null, zone: FocusZone): HTMLElement | null {
  if (!root || isModalNavigationRoot(root)) {
    return null;
  }

  const explicitRoot = root.querySelector<HTMLElement>(`[${FOCUS_ZONE_ATTRIBUTE}="${zone}"]`);
  if (explicitRoot) {
    return explicitRoot;
  }

  return root.querySelector<HTMLElement>(zone === 'sidebar' ? SIDEBAR_FALLBACK_SELECTOR : CONTENT_FALLBACK_SELECTOR);
}

export function getFocusZoneForElement(root: HTMLElement | null, element: HTMLElement | null): FocusZone | null {
  if (!root || !element || isModalNavigationRoot(root)) {
    return null;
  }

  const explicitZoneRoot = element.closest<HTMLElement>(`[${FOCUS_ZONE_ATTRIBUTE}]`);
  const explicitZone = explicitZoneRoot?.getAttribute(FOCUS_ZONE_ATTRIBUTE);

  if ((explicitZone === 'sidebar' || explicitZone === 'content') && root.contains(explicitZoneRoot)) {
    return explicitZone;
  }

  const sidebarRoot = getFocusZoneRoot(root, 'sidebar');
  if (sidebarRoot?.contains(element)) {
    return 'sidebar';
  }

  const contentRoot = getFocusZoneRoot(root, 'content');
  if (contentRoot?.contains(element)) {
    return 'content';
  }

  return null;
}

export function getFocusableElements(root: HTMLElement | null): HTMLElement[] {
  if (!root) {
    return [];
  }

  return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(isFocusable);
}

export function focusElement(element: HTMLElement | null, scrollIntoView = true): void {
  if (!element) {
    return;
  }

  element.focus({ preventScroll: !scrollIntoView });

  if (scrollIntoView) {
    element.scrollIntoView({
      block: 'nearest',
      inline: 'nearest',
    });
  }
}

export function getCurrentIndex(elements: HTMLElement[], activeElement: Element | null): number {
  if (!activeElement) {
    return -1;
  }

  const index = elements.indexOf(activeElement as HTMLElement);
  if (index >= 0) {
    return index;
  }

  const activeAncestor = elements.find((element) => element.contains(activeElement));
  return activeAncestor ? elements.indexOf(activeAncestor) : -1;
}

export function isEditableElement(element: EventTarget | null): boolean {
  if (!(element instanceof HTMLElement)) {
    return false;
  }

  if (element.isContentEditable) {
    return true;
  }

  if (element instanceof HTMLTextAreaElement || element instanceof HTMLSelectElement) {
    return true;
  }

  if (element instanceof HTMLInputElement) {
    switch (element.type) {
      case 'button':
      case 'checkbox':
      case 'color':
      case 'file':
      case 'image':
      case 'radio':
      case 'range':
      case 'reset':
      case 'submit':
        return false;
      default:
        return true;
    }
  }

  return false;
}
