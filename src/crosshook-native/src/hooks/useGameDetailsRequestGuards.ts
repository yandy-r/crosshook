import { type MutableRefObject, useRef } from 'react';

/**
 * Monotonic counter for modal-scoped async work. Capture `next()` when starting an
 * operation; after `await`, drop results when `id !== current()`.
 */
export function useGameDetailsRequestCounter() {
  return useRef(0);
}

export function nextGameDetailsRequestId(counter: MutableRefObject<number>): number {
  counter.current += 1;
  return counter.current;
}
