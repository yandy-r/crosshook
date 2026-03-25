import { useCallback, useEffect, useRef, useState, type MutableRefObject } from 'react';

export interface GamepadNavOptions {
  enabled?: boolean;
  onConfirm?: (element: HTMLElement | null) => void;
  onBack?: () => void;
  onFocusChange?: (element: HTMLElement | null) => void;
  wrap?: boolean;
}

export interface GamepadNavState {
  rootRef: MutableRefObject<HTMLElement | null>;
  controllerMode: boolean;
  isSteamDeck: boolean;
  activeElement: HTMLElement | null;
  activeIndex: number;
  setControllerMode: (enabled: boolean) => void;
  focusFirst: () => void;
  focusLast: () => void;
  focusNext: () => void;
  focusPrevious: () => void;
  confirm: () => void;
  back: () => void;
}

const FOCUSABLE_SELECTOR = [
  'button:not([disabled])',
  '[href]',
  'input:not([disabled])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  "[tabindex]:not([tabindex='-1'])",
  "[contenteditable='true']",
  'summary',
].join(',');

const GAMEPAD_CONFIRM_BUTTON = 0;
const GAMEPAD_BACK_BUTTON = 1;
const GAMEPAD_DPAD_UP = 12;
const GAMEPAD_DPAD_DOWN = 13;
const GAMEPAD_DPAD_LEFT = 14;
const GAMEPAD_DPAD_RIGHT = 15;
const AXIS_ACTIVATION_THRESHOLD = 0.5;
const MODAL_FOCUS_ROOT_SELECTOR = '[data-crosshook-focus-root="modal"]';

function isVisible(element: HTMLElement): boolean {
  const style = window.getComputedStyle(element);
  return (
    style.display !== 'none' &&
    style.visibility !== 'hidden' &&
    style.opacity !== '0' &&
    element.getClientRects().length > 0
  );
}

function isFocusable(element: HTMLElement): boolean {
  if (element.hasAttribute('disabled') || element.getAttribute('aria-hidden') === 'true') {
    return false;
  }

  const tabIndex = element.getAttribute('tabindex');
  if (tabIndex === '-1') {
    return false;
  }

  return isVisible(element);
}

function getRootElement(rootRef: MutableRefObject<HTMLElement | null>): HTMLElement | null {
  return rootRef.current;
}

function getNavigationRoot(rootRef: MutableRefObject<HTMLElement | null>): HTMLElement | null {
  const modalRoots = document.querySelectorAll<HTMLElement>(MODAL_FOCUS_ROOT_SELECTOR);
  return modalRoots.item(modalRoots.length - 1) ?? getRootElement(rootRef);
}

function getFocusableElements(root: HTMLElement | null): HTMLElement[] {
  if (!root) {
    return [];
  }

  return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(isFocusable);
}

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

function isSteamDeckRuntime(): boolean {
  if (typeof window === 'undefined') {
    return false;
  }

  const globalCandidate = window as Window &
    typeof globalThis & {
      SteamDeck?: string | number | boolean;
      STEAM_DECK?: string | number | boolean;
    };

  const flagValues = [
    globalCandidate.SteamDeck,
    globalCandidate.STEAM_DECK,
    (import.meta as ImportMeta & { env?: Record<string, string | undefined> }).env?.SteamDeck,
    (import.meta as ImportMeta & { env?: Record<string, string | undefined> }).env?.STEAM_DECK,
    (import.meta as ImportMeta & { env?: Record<string, string | undefined> }).env?.VITE_STEAM_DECK,
  ];

  if (
    flagValues.some((value) => value === true || value === 1 || value === '1' || value === 'true' || value === 'TRUE')
  ) {
    return true;
  }

  const coarsePointer = window.matchMedia?.('(pointer: coarse)').matches ?? false;
  const handheldViewport = window.matchMedia?.('(max-width: 1280px) and (max-height: 800px)').matches ?? false;
  const userAgent = window.navigator.userAgent.toLowerCase();

  return coarsePointer && handheldViewport && userAgent.includes('steam');
}

function focusElement(element: HTMLElement | null, scrollIntoView = true): void {
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

function getCurrentIndex(elements: HTMLElement[], activeElement: Element | null): number {
  if (!activeElement) {
    return -1;
  }

  const index = elements.findIndex((element) => element === activeElement);
  if (index >= 0) {
    return index;
  }

  const activeAncestor = elements.find((element) => element.contains(activeElement));
  return activeAncestor ? elements.indexOf(activeAncestor) : -1;
}

function isEditableElement(element: EventTarget | null): boolean {
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

export function useGamepadNav(options: GamepadNavOptions = {}): GamepadNavState {
  const rootRef = useRef<HTMLElement | null>(null);
  const [controllerMode, setControllerMode] = useState(() => {
    if (typeof options.enabled === 'boolean') {
      return options.enabled;
    }

    return isSteamDeckRuntime();
  });
  const [activeElement, setActiveElement] = useState<HTMLElement | null>(null);
  const [activeIndex, setActiveIndex] = useState(-1);
  const lastGamepadState = useRef(new Map<number, { buttons: boolean[]; axes: number[] }>());

  useEffect(() => {
    if (typeof options.enabled === 'boolean') {
      setControllerMode(options.enabled);
      return;
    }

    setControllerMode(isSteamDeckRuntime());
  }, [options.enabled]);

  useEffect(() => {
    if (typeof document === 'undefined') {
      return;
    }

    const controllerAttr = 'data-crosshook-controller-mode';

    if (controllerMode) {
      document.documentElement.setAttribute(controllerAttr, 'true');
    } else {
      document.documentElement.removeAttribute(controllerAttr);
    }

    return () => {
      document.documentElement.removeAttribute(controllerAttr);
    };
  }, [controllerMode]);

  const updateActiveState = useCallback(() => {
    const root = getNavigationRoot(rootRef);
    const focusables = getFocusableElements(root);
    const current = document.activeElement;
    const index = getCurrentIndex(focusables, current);
    setActiveIndex(index);
    setActiveElement(index >= 0 ? focusables[index] : current instanceof HTMLElement ? current : null);
  }, []);

  const focusByIndex = useCallback(
    (index: number) => {
      const root = getNavigationRoot(rootRef);
      const focusables = getFocusableElements(root);
      if (focusables.length === 0) {
        return;
      }

      const boundedIndex = clampIndex(index, focusables.length);
      if (boundedIndex < 0) {
        return;
      }

      const target = focusables[boundedIndex];
      focusElement(target);
      setActiveIndex(boundedIndex);
      setActiveElement(target);
      options.onFocusChange?.(target);
    },
    [options]
  );

  const focusFirst = useCallback(() => focusByIndex(0), [focusByIndex]);
  const focusLast = useCallback(() => {
    const root = getNavigationRoot(rootRef);
    const focusables = getFocusableElements(root);
    if (focusables.length === 0) {
      return;
    }

    focusByIndex(focusables.length - 1);
  }, [focusByIndex]);
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
  }, [focusByIndex, options.wrap]);
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
  }, [focusByIndex, options.wrap]);

  const confirm = useCallback(() => {
    const current = document.activeElement;
    if (current instanceof HTMLElement) {
      current.click();
    }

    options.onConfirm?.(current instanceof HTMLElement ? current : null);
  }, [options]);

  const back = useCallback(() => {
    options.onBack?.();
  }, [options]);

  useEffect(() => {
    const handleFocusIn = () => {
      updateActiveState();
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      const root = getRootElement(rootRef);
      if (!root || !root.contains(document.activeElement)) {
        return;
      }

      if (isEditableElement(event.target)) {
        return;
      }

      switch (event.key) {
        case 'Tab':
          event.preventDefault();
          if (event.shiftKey) {
            focusPrevious();
          } else {
            focusNext();
          }
          break;
        case 'ArrowDown':
        case 'ArrowRight':
          event.preventDefault();
          focusNext();
          break;
        case 'ArrowUp':
        case 'ArrowLeft':
          event.preventDefault();
          focusPrevious();
          break;
        case 'Enter':
        case ' ':
          event.preventDefault();
          confirm();
          break;
        case 'Escape':
          event.preventDefault();
          back();
          break;
        default:
          break;
      }
    };

    document.addEventListener('focusin', handleFocusIn);
    document.addEventListener('keydown', handleKeyDown, true);

    return () => {
      document.removeEventListener('focusin', handleFocusIn);
      document.removeEventListener('keydown', handleKeyDown, true);
    };
  }, [back, confirm, focusNext, focusPrevious, updateActiveState]);

  useEffect(() => {
    if (!controllerMode || typeof window === 'undefined') {
      return undefined;
    }

    let frameId = 0;
    let lastActiveGamepadIndex = -1;

    const poll = () => {
      const gamepads = navigator.getGamepads?.() ?? [];

      for (const gamepad of gamepads) {
        if (!gamepad || !gamepad.connected) {
          continue;
        }

        const previousState = lastGamepadState.current.get(gamepad.index) ?? {
          buttons: [],
          axes: [],
        };
        const buttonPressed = (buttonIndex: number): boolean => Boolean(gamepad.buttons[buttonIndex]?.pressed);
        const buttonWasPressed = (buttonIndex: number): boolean => Boolean(previousState.buttons[buttonIndex]);
        const edge = (buttonIndex: number): boolean => buttonPressed(buttonIndex) && !buttonWasPressed(buttonIndex);

        if (edge(GAMEPAD_DPAD_DOWN) || edge(GAMEPAD_DPAD_RIGHT)) {
          focusNext();
        } else if (edge(GAMEPAD_DPAD_UP) || edge(GAMEPAD_DPAD_LEFT)) {
          focusPrevious();
        } else if (edge(GAMEPAD_CONFIRM_BUTTON)) {
          confirm();
        } else if (edge(GAMEPAD_BACK_BUTTON)) {
          back();
        } else {
          const vertical = gamepad.axes[1] ?? 0;
          const horizontal = gamepad.axes[0] ?? 0;
          const movedDown =
            vertical > AXIS_ACTIVATION_THRESHOLD && (previousState.axes[1] ?? 0) <= AXIS_ACTIVATION_THRESHOLD;
          const movedUp =
            vertical < -AXIS_ACTIVATION_THRESHOLD && (previousState.axes[1] ?? 0) >= -AXIS_ACTIVATION_THRESHOLD;
          const movedRight =
            horizontal > AXIS_ACTIVATION_THRESHOLD && (previousState.axes[0] ?? 0) <= AXIS_ACTIVATION_THRESHOLD;
          const movedLeft =
            horizontal < -AXIS_ACTIVATION_THRESHOLD && (previousState.axes[0] ?? 0) >= -AXIS_ACTIVATION_THRESHOLD;

          if (movedDown || movedRight) {
            focusNext();
          } else if (movedUp || movedLeft) {
            focusPrevious();
          }
        }

        lastGamepadState.current.set(gamepad.index, {
          buttons: gamepad.buttons.map((button) => button.pressed),
          axes: gamepad.axes.slice(),
        });

        lastActiveGamepadIndex = gamepad.index;
        break;
      }

      for (const index of lastGamepadState.current.keys()) {
        if (index !== lastActiveGamepadIndex) {
          lastGamepadState.current.delete(index);
        }
      }

      frameId = window.requestAnimationFrame(poll);
    };

    frameId = window.requestAnimationFrame(poll);

    return () => {
      window.cancelAnimationFrame(frameId);
    };
  }, [back, confirm, controllerMode, focusNext, focusPrevious]);

  useEffect(() => {
    updateActiveState();
  }, [updateActiveState]);

  return {
    rootRef,
    controllerMode,
    isSteamDeck: isSteamDeckRuntime(),
    activeElement,
    activeIndex,
    setControllerMode,
    focusFirst,
    focusLast,
    focusNext,
    focusPrevious,
    confirm,
    back,
  };
}
