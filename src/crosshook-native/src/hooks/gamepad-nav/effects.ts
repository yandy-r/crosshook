import { type MutableRefObject, useEffect } from 'react';

import {
  AXIS_ACTIVATION_THRESHOLD,
  GAMEPAD_BACK_BUTTON,
  GAMEPAD_CONFIRM_BUTTON,
  GAMEPAD_DPAD_DOWN,
  GAMEPAD_DPAD_LEFT,
  GAMEPAD_DPAD_RIGHT,
  GAMEPAD_DPAD_UP,
  GAMEPAD_LEFT_BUMPER,
  GAMEPAD_RIGHT_BUMPER,
} from './constants';
import { getFocusZoneRoot, getNavigationRoot, getRootElement, isEditableElement, isModalNavigationRoot } from './dom';
import type { FocusZone, GamepadStateMap } from './types';

interface GamepadNavEffectsArgs {
  rootRef: MutableRefObject<HTMLElement | null>;
  controllerMode: boolean;
  focusNext: () => void;
  focusPrevious: () => void;
  confirm: () => void;
  back: () => void;
  switchZone: (zone: FocusZone) => boolean;
  cycleSidebarView: (direction: -1 | 1) => boolean;
  updateActiveState: () => void;
  focusZone: (zone: FocusZone, preference?: 'remembered' | 'first' | 'last', scrollIntoView?: boolean) => boolean;
  lastGamepadState: MutableRefObject<GamepadStateMap>;
  lastSidebarRouteRef: MutableRefObject<string | null>;
}

function useFocusAndKeyboardHandlers({
  rootRef,
  focusNext,
  focusPrevious,
  confirm,
  back,
  updateActiveState,
}: Pick<GamepadNavEffectsArgs, 'rootRef' | 'focusNext' | 'focusPrevious' | 'confirm' | 'back' | 'updateActiveState'>) {
  useEffect(() => {
    const handleFocusIn = () => {
      updateActiveState();
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      const root = getRootElement(rootRef);
      if (!root?.contains(document.activeElement)) {
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
  }, [back, confirm, focusNext, focusPrevious, rootRef, updateActiveState]);
}

function useSidebarFocusSync({
  rootRef,
  controllerMode,
  focusZone,
  lastSidebarRouteRef,
}: Pick<GamepadNavEffectsArgs, 'rootRef' | 'controllerMode' | 'focusZone' | 'lastSidebarRouteRef'>) {
  useEffect(() => {
    const root = getRootElement(rootRef);
    if (!root) {
      return;
    }

    const getActiveSidebarValue = (): string | null => {
      const sidebarRoot = getFocusZoneRoot(root, 'sidebar');
      return (
        sidebarRoot?.querySelector<HTMLElement>('[data-state="active"]')?.getAttribute('value') ??
        sidebarRoot?.querySelector<HTMLElement>('[aria-current="page"]')?.getAttribute('value') ??
        null
      );
    };

    lastSidebarRouteRef.current = getActiveSidebarValue();

    const observer = new MutationObserver(() => {
      const nextRoute = getActiveSidebarValue();
      if (nextRoute === null || nextRoute === lastSidebarRouteRef.current) {
        return;
      }

      lastSidebarRouteRef.current = nextRoute;

      if (!controllerMode || isModalNavigationRoot(getNavigationRoot(rootRef))) {
        return;
      }

      window.requestAnimationFrame(() => {
        void focusZone('content', 'first', false);
      });
    });

    observer.observe(root, {
      attributes: true,
      attributeFilter: ['data-state', 'aria-current', 'value'],
      subtree: true,
    });

    return () => {
      observer.disconnect();
    };
  }, [controllerMode, focusZone, lastSidebarRouteRef, rootRef]);
}

function useGamepadPolling({
  controllerMode,
  focusNext,
  focusPrevious,
  switchZone,
  cycleSidebarView,
  confirm,
  back,
  lastGamepadState,
}: Pick<
  GamepadNavEffectsArgs,
  | 'controllerMode'
  | 'focusNext'
  | 'focusPrevious'
  | 'switchZone'
  | 'cycleSidebarView'
  | 'confirm'
  | 'back'
  | 'lastGamepadState'
>) {
  useEffect(() => {
    if (!controllerMode || typeof window === 'undefined') {
      return undefined;
    }

    let frameId = 0;
    let lastActiveGamepadIndex = -1;

    const poll = () => {
      const gamepads = navigator.getGamepads?.() ?? [];

      for (const gamepad of gamepads) {
        if (!gamepad?.connected) {
          continue;
        }

        const previousState = lastGamepadState.current.get(gamepad.index) ?? {
          buttons: [],
          axes: [],
        };
        const buttonPressed = (buttonIndex: number): boolean => Boolean(gamepad.buttons[buttonIndex]?.pressed);
        const buttonWasPressed = (buttonIndex: number): boolean => Boolean(previousState.buttons[buttonIndex]);
        const edge = (buttonIndex: number): boolean => buttonPressed(buttonIndex) && !buttonWasPressed(buttonIndex);

        if (edge(GAMEPAD_DPAD_DOWN)) {
          focusNext();
        } else if (edge(GAMEPAD_DPAD_UP)) {
          focusPrevious();
        } else if (edge(GAMEPAD_DPAD_LEFT)) {
          if (!switchZone('sidebar')) {
            focusPrevious();
          }
        } else if (edge(GAMEPAD_DPAD_RIGHT)) {
          if (!switchZone('content')) {
            focusNext();
          }
        } else if (edge(GAMEPAD_LEFT_BUMPER)) {
          cycleSidebarView(-1);
        } else if (edge(GAMEPAD_RIGHT_BUMPER)) {
          cycleSidebarView(1);
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

          if (movedDown) {
            focusNext();
          } else if (movedUp) {
            focusPrevious();
          } else if (movedLeft) {
            if (!switchZone('sidebar')) {
              focusPrevious();
            }
          } else if (movedRight) {
            if (!switchZone('content')) {
              focusNext();
            }
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
  }, [back, confirm, controllerMode, cycleSidebarView, focusNext, focusPrevious, lastGamepadState, switchZone]);
}

function useInitialActiveState(updateActiveState: () => void) {
  useEffect(() => {
    updateActiveState();
  }, [updateActiveState]);
}

export function useGamepadNavEffects(args: GamepadNavEffectsArgs): void {
  useFocusAndKeyboardHandlers(args);
  useSidebarFocusSync(args);
  useGamepadPolling(args);
  useInitialActiveState(args.updateActiveState);
}
