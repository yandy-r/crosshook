import { type MutableRefObject, useCallback } from 'react';

import {
  focusElement,
  getCurrentIndex,
  getFocusableElements,
  getFocusZoneForElement,
  getFocusZoneRoot,
  getNavigationRoot,
  getRootElement,
  isFocusable,
  isModalNavigationRoot,
} from './dom';
import type { FocusZone, GamepadNavOptions } from './types';

function clampIndex(index: number, length: number): number {
  if (length <= 0) {
    return -1;
  }

  if (index < 0) {
    return 0;
  }

  if (index >= length) {
    return length - 1;
  }

  return index;
}

interface FocusManagementArgs {
  rootRef: MutableRefObject<HTMLElement | null>;
  activeZoneRef: MutableRefObject<FocusZone | null>;
  lastFocusedByZoneRef: MutableRefObject<Partial<Record<FocusZone, HTMLElement>>>;
  options: GamepadNavOptions;
  setActiveElement: (element: HTMLElement | null) => void;
  setActiveIndex: (index: number) => void;
}

export function useFocusManagement({
  rootRef,
  activeZoneRef,
  lastFocusedByZoneRef,
  options,
  setActiveElement,
  setActiveIndex,
}: FocusManagementArgs) {
  const updateActiveState = useCallback(() => {
    const root = getNavigationRoot(rootRef);
    const current = document.activeElement;
    const currentElement = current instanceof HTMLElement ? current : null;
    const focusZone = getFocusZoneForElement(getRootElement(rootRef), currentElement);

    if (focusZone) {
      activeZoneRef.current = focusZone;
      if (currentElement && document.contains(currentElement) && isFocusable(currentElement)) {
        lastFocusedByZoneRef.current[focusZone] = currentElement;
      }
    }

    const zoneRoot = focusZone ? getFocusZoneRoot(getRootElement(rootRef), focusZone) : null;
    const focusables = getFocusableElements(zoneRoot ?? root);
    const index = getCurrentIndex(focusables, current);
    setActiveIndex(index);
    setActiveElement(index >= 0 ? focusables[index] : currentElement);
  }, [activeZoneRef, lastFocusedByZoneRef, rootRef, setActiveElement, setActiveIndex]);

  const getCurrentZone = useCallback((): FocusZone | null => {
    const root = getRootElement(rootRef);
    const active = document.activeElement;

    if (active instanceof HTMLElement) {
      const zone = getFocusZoneForElement(root, active);
      if (zone) {
        activeZoneRef.current = zone;
        return zone;
      }
    }

    return activeZoneRef.current;
  }, [activeZoneRef, rootRef]);

  const focusZone = useCallback(
    (zone: FocusZone, preference: 'remembered' | 'first' | 'last' = 'remembered', scrollIntoView = true): boolean => {
      const root = getRootElement(rootRef);
      const zoneRoot = getFocusZoneRoot(root, zone);

      if (!zoneRoot) {
        return false;
      }

      const focusables = getFocusableElements(zoneRoot);
      if (focusables.length === 0) {
        return false;
      }

      const rememberedElement = lastFocusedByZoneRef.current[zone];
      const rememberedIndex =
        rememberedElement && zoneRoot.contains(rememberedElement) ? focusables.indexOf(rememberedElement) : -1;

      let targetIndex = rememberedIndex;
      if (preference === 'first') {
        targetIndex = 0;
      } else if (preference === 'last') {
        targetIndex = focusables.length - 1;
      } else if (rememberedIndex < 0) {
        targetIndex = 0;
      }

      const target = focusables[targetIndex];
      if (!target) {
        return false;
      }

      focusElement(target, scrollIntoView);
      activeZoneRef.current = zone;
      lastFocusedByZoneRef.current[zone] = target;
      setActiveIndex(targetIndex);
      setActiveElement(target);
      options.onFocusChange?.(target);

      return true;
    },
    [activeZoneRef, lastFocusedByZoneRef, options, rootRef, setActiveElement, setActiveIndex]
  );

  const switchZone = useCallback(
    (zone: FocusZone): boolean => {
      const root = getRootElement(rootRef);
      if (!getFocusZoneRoot(root, zone)) {
        return false;
      }

      return focusZone(zone, 'remembered');
    },
    [focusZone, rootRef]
  );

  const cycleSidebarView = useCallback(
    (direction: -1 | 1): boolean => {
      const root = getRootElement(rootRef);
      const sidebarRoot = getFocusZoneRoot(root, 'sidebar');

      if (!sidebarRoot) {
        return false;
      }

      const focusables = getFocusableElements(sidebarRoot);
      if (focusables.length === 0) {
        return false;
      }

      const activeElement = document.activeElement;
      const activeTrigger =
        sidebarRoot.querySelector<HTMLElement>('[data-state="active"]') ??
        sidebarRoot.querySelector<HTMLElement>('[aria-current="page"]');
      const currentIndex = getCurrentIndex(
        focusables,
        activeElement instanceof HTMLElement && sidebarRoot.contains(activeElement) ? activeElement : activeTrigger
      );
      const baseIndex = currentIndex >= 0 ? currentIndex : 0;
      const targetIndex =
        direction > 0 ? (baseIndex + 1) % focusables.length : (baseIndex - 1 + focusables.length) % focusables.length;
      const target = focusables[targetIndex];

      if (!target) {
        return false;
      }

      focusElement(target);
      target.click();
      activeZoneRef.current = 'sidebar';
      lastFocusedByZoneRef.current.sidebar = target;
      setActiveIndex(targetIndex);
      setActiveElement(target);
      options.onFocusChange?.(target);

      return true;
    },
    [activeZoneRef, lastFocusedByZoneRef, options, rootRef, setActiveElement, setActiveIndex]
  );

  const focusByIndex = useCallback(
    (index: number) => {
      const navigationRoot = getNavigationRoot(rootRef);
      const activeZone = getCurrentZone();
      const focusRoot =
        activeZone && !isModalNavigationRoot(navigationRoot)
          ? (getFocusZoneRoot(getRootElement(rootRef), activeZone) ?? navigationRoot)
          : navigationRoot;
      const focusables = getFocusableElements(focusRoot);
      if (focusables.length === 0) {
        return;
      }

      const boundedIndex = clampIndex(index, focusables.length);
      if (boundedIndex < 0) {
        return;
      }

      const target = focusables[boundedIndex];
      const targetZone = getFocusZoneForElement(getRootElement(rootRef), target);
      focusElement(target);
      if (targetZone) {
        activeZoneRef.current = targetZone;
        lastFocusedByZoneRef.current[targetZone] = target;
      }
      setActiveIndex(boundedIndex);
      setActiveElement(target);
      options.onFocusChange?.(target);
    },
    [activeZoneRef, getCurrentZone, lastFocusedByZoneRef, options, rootRef, setActiveElement, setActiveIndex]
  );

  const focusFirst = useCallback(() => focusByIndex(0), [focusByIndex]);

  const focusLast = useCallback(() => {
    const root = getNavigationRoot(rootRef);
    const focusables = getFocusableElements(root);
    if (focusables.length === 0) {
      return;
    }

    focusByIndex(focusables.length - 1);
  }, [focusByIndex, rootRef]);

  const focusNext = useCallback(() => {
    const root = getNavigationRoot(rootRef);
    const focusables = getFocusableElements(root);
    if (focusables.length === 0) {
      return;
    }

    const currentIndex = getCurrentIndex(focusables, document.activeElement);
    const nextIndex = currentIndex < 0 ? 0 : currentIndex + 1;
    const targetIndex = nextIndex >= focusables.length ? (options.wrap === false ? currentIndex : 0) : nextIndex;

    if (targetIndex >= 0) {
      focusByIndex(targetIndex);
    }
  }, [focusByIndex, options.wrap, rootRef]);

  const focusPrevious = useCallback(() => {
    const root = getNavigationRoot(rootRef);
    const focusables = getFocusableElements(root);
    if (focusables.length === 0) {
      return;
    }

    const currentIndex = getCurrentIndex(focusables, document.activeElement);
    const previousIndex = currentIndex < 0 ? focusables.length - 1 : currentIndex - 1;
    const targetIndex =
      previousIndex < 0 ? (options.wrap === false ? currentIndex : focusables.length - 1) : previousIndex;

    if (targetIndex >= 0) {
      focusByIndex(targetIndex);
    }
  }, [focusByIndex, options.wrap, rootRef]);

  const confirm = useCallback(() => {
    const current = document.activeElement;
    if (current instanceof HTMLElement) {
      current.click();
    }

    options.onConfirm?.(current instanceof HTMLElement ? current : null);
  }, [options]);

  const back = useCallback(() => {
    const navigationRoot = getNavigationRoot(rootRef);
    if (!isModalNavigationRoot(navigationRoot)) {
      const root = getRootElement(rootRef);
      const currentZone = getCurrentZone();

      if (currentZone === 'content' && getFocusZoneRoot(root, 'sidebar') && switchZone('sidebar')) {
        return;
      }

      if (currentZone === 'sidebar' && getFocusZoneRoot(root, 'sidebar')) {
        return;
      }
    }

    options.onBack?.();
  }, [getCurrentZone, options, rootRef, switchZone]);

  return {
    updateActiveState,
    getCurrentZone,
    focusZone,
    switchZone,
    cycleSidebarView,
    focusByIndex,
    focusFirst,
    focusLast,
    focusNext,
    focusPrevious,
    confirm,
    back,
  };
}
