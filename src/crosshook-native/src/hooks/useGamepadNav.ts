import { useEffect, useRef, useState } from 'react';

import { useGamepadNavEffects } from './gamepad-nav/effects';
import { useFocusManagement } from './gamepad-nav/focusManagement';
import { isSteamDeckRuntime } from './gamepad-nav/steamDeck';
import type { FocusZone, GamepadNavOptions, GamepadNavState, GamepadStateMap } from './gamepad-nav/types';

export { isSteamDeckRuntime } from './gamepad-nav/steamDeck';
export type { GamepadNavOptions, GamepadNavState } from './gamepad-nav/types';

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
  const lastGamepadState = useRef<GamepadStateMap>(new Map());
  const activeZoneRef = useRef<FocusZone | null>(null);
  const lastFocusedByZoneRef = useRef<Partial<Record<FocusZone, HTMLElement>>>({});
  const lastSidebarRouteRef = useRef<string | null>(null);

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

  const focusManagement = useFocusManagement({
    rootRef,
    activeZoneRef,
    lastFocusedByZoneRef,
    options,
    setActiveElement,
    setActiveIndex,
  });

  useGamepadNavEffects({
    rootRef,
    controllerMode,
    focusNext: focusManagement.focusNext,
    focusPrevious: focusManagement.focusPrevious,
    confirm: focusManagement.confirm,
    back: focusManagement.back,
    switchZone: focusManagement.switchZone,
    cycleSidebarView: focusManagement.cycleSidebarView,
    updateActiveState: focusManagement.updateActiveState,
    focusZone: focusManagement.focusZone,
    lastGamepadState,
    lastSidebarRouteRef,
  });

  return {
    rootRef,
    controllerMode,
    isSteamDeck: isSteamDeckRuntime(),
    activeElement,
    activeIndex,
    setControllerMode,
    focusFirst: focusManagement.focusFirst,
    focusLast: focusManagement.focusLast,
    focusNext: focusManagement.focusNext,
    focusPrevious: focusManagement.focusPrevious,
    confirm: focusManagement.confirm,
    back: focusManagement.back,
  };
}
