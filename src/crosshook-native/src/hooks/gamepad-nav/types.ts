import type { MutableRefObject } from 'react';

export type FocusZone = 'sidebar' | 'content';

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

export type GamepadStateMap = Map<number, { buttons: boolean[]; axes: number[] }>;
