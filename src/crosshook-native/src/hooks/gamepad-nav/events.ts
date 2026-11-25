/**
 * Fan-out for gamepad buttons that mean different things per page.
 *
 * The gamepad-nav poll (`effects.ts`) is the only code that reads gamepad
 * state, and only while controller mode is on. It edge-detects a button press
 * once and emits it here; pages subscribe instead of running a private
 * `requestAnimationFrame` poll of `navigator.getGamepads()` themselves.
 */

type GamepadActionListener = () => void;

const yButtonListeners = new Set<GamepadActionListener>();

/** Notifies every Y-action listener. Call once per rising edge of the Y button. */
export function emitGamepadYAction(): void {
  // Snapshot so a listener that unsubscribes (or subscribes) mid-emit cannot
  // change which listeners this emission reaches, and isolate throws so one
  // faulty subscriber cannot kill the gamepad poll loop.
  for (const listener of [...yButtonListeners]) {
    try {
      listener();
    } catch (error) {
      console.error('gamepad Y listener failed:', error);
    }
  }
}

/** Registers `listener` for the Y action and returns a function that removes it again. */
export function subscribeGamepadYAction(listener: GamepadActionListener): () => void {
  yButtonListeners.add(listener);
  return () => {
    yButtonListeners.delete(listener);
  };
}
