export const FOCUSABLE_SELECTOR = [
  'button:not([disabled])',
  '[href]',
  'input:not([disabled])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  "[tabindex]:not([tabindex='-1'])",
  "[contenteditable='true']",
  'summary',
].join(',');

export const GAMEPAD_CONFIRM_BUTTON = 0;
export const GAMEPAD_BACK_BUTTON = 1;
export const GAMEPAD_LEFT_BUMPER = 4;
export const GAMEPAD_RIGHT_BUMPER = 5;
export const GAMEPAD_DPAD_UP = 12;
export const GAMEPAD_DPAD_DOWN = 13;
export const GAMEPAD_DPAD_LEFT = 14;
export const GAMEPAD_DPAD_RIGHT = 15;
export const AXIS_ACTIVATION_THRESHOLD = 0.5;
export const MODAL_FOCUS_ROOT_SELECTOR = '[data-crosshook-focus-root="modal"]';
export const FOCUS_ZONE_ATTRIBUTE = 'data-crosshook-focus-zone';
export const SIDEBAR_FALLBACK_SELECTOR = '.crosshook-sidebar';
export const CONTENT_FALLBACK_SELECTOR = '.crosshook-content-area';
