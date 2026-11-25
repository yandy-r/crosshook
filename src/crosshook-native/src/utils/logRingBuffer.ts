/**
 * Line cap for the runtime console. The view renders at most this many lines;
 * anything older is counted in `droppedCount` and shown as a summary.
 */
export const CONSOLE_MAX_LINES = 2000;

export interface LogRingBufferState<T> {
  /** Newest lines, in arrival order; at most `maxLines` entries. */
  lines: readonly T[];
  /** Total lines discarded so far because they no longer fit under the cap. */
  droppedCount: number;
}

/**
 * Shared empty state. Never mutate it: `appendLogRingBuffer` treats its inputs
 * as read-only and returns a new state object for every change.
 */
export const EMPTY_LOG_RING_BUFFER: LogRingBufferState<never> = { lines: [], droppedCount: 0 };

/**
 * Appends `incoming` to a bounded log buffer without mutating either argument.
 *
 * `preDropped` adds lines the caller already discarded upstream (for example a
 * pending queue trimmed before a flush) so the visible total stays exact.
 * Returns `state` itself when there is nothing to add, so a caller can bail out
 * of a re-render on an empty flush. O(maxLines) worst case, independent of how
 * large `incoming` is.
 */
export function appendLogRingBuffer<T>(
  state: LogRingBufferState<T>,
  incoming: readonly T[],
  maxLines = CONSOLE_MAX_LINES,
  preDropped = 0
): LogRingBufferState<T> {
  if (incoming.length === 0 && preDropped === 0) {
    return state;
  }

  // The batch alone fills the cap: everything buffered before it is dropped.
  if (incoming.length >= maxLines) {
    return {
      lines: incoming.slice(incoming.length - maxLines),
      droppedCount: state.droppedCount + preDropped + state.lines.length + (incoming.length - maxLines),
    };
  }

  const overflow = state.lines.length + incoming.length - maxLines;
  if (overflow > 0) {
    return {
      lines: [...state.lines.slice(overflow), ...incoming],
      droppedCount: state.droppedCount + preDropped + overflow,
    };
  }

  return {
    lines: [...state.lines, ...incoming],
    droppedCount: state.droppedCount + preDropped,
  };
}
