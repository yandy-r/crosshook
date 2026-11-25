import { describe, expect, it } from 'vitest';
import {
  appendLogRingBuffer,
  CONSOLE_MAX_LINES,
  EMPTY_LOG_RING_BUFFER,
  type LogRingBufferState,
} from '../logRingBuffer';

type Line = { id: number };

function line(id: number): Line {
  return { id };
}

function linesFrom(start: number, count: number): Line[] {
  return Array.from({ length: count }, (_, index) => line(start + index));
}

/** Drains `batches` through the buffer one append at a time. */
function appendAll(state: LogRingBufferState<Line>, batches: readonly (readonly Line[])[]): LogRingBufferState<Line> {
  let current = state;
  for (const batch of batches) {
    current = appendLogRingBuffer(current, batch);
  }
  return current;
}

function ids(state: LogRingBufferState<Line>): number[] {
  return state.lines.map((entry) => entry.id);
}

describe('appendLogRingBuffer', () => {
  it('keeps lines in arrival order while under the cap and reports nothing dropped', () => {
    const state = appendAll(EMPTY_LOG_RING_BUFFER, [linesFrom(0, 3), linesFrom(3, 2)]);

    expect(ids(state)).toEqual([0, 1, 2, 3, 4]);
    expect(state.droppedCount).toBe(0);
  });

  it('caps the buffer at 2,000 lines', () => {
    expect(CONSOLE_MAX_LINES).toBe(2000);

    const state = appendLogRingBuffer(EMPTY_LOG_RING_BUFFER, linesFrom(0, CONSOLE_MAX_LINES));

    expect(state.lines).toHaveLength(2000);
    expect(state.droppedCount).toBe(0);
    expect(state.lines[0]).toEqual(line(0));
    expect(state.lines[1999]).toEqual(line(1999));
  });

  it('returns the same state object when there is nothing to append', () => {
    const state = appendLogRingBuffer(EMPTY_LOG_RING_BUFFER, linesFrom(0, 2));

    expect(appendLogRingBuffer(state, [])).toBe(state);
    expect(appendLogRingBuffer(EMPTY_LOG_RING_BUFFER, [])).toBe(EMPTY_LOG_RING_BUFFER);
  });

  it('drops exactly the oldest lines one batch over the cap', () => {
    // 1,998 buffered + 5 arriving = 3 over the cap.
    const state = appendAll(EMPTY_LOG_RING_BUFFER, [linesFrom(0, 1998), linesFrom(1998, 5)]);

    expect(state.lines).toHaveLength(2000);
    expect(state.droppedCount).toBe(3);
    expect(ids(state)).toEqual(ids({ lines: linesFrom(3, 2000), droppedCount: 0 }));
  });

  it('accumulates an exact dropped count across incremental batches', () => {
    const state = appendAll(EMPTY_LOG_RING_BUFFER, [
      linesFrom(0, 1998), // 1,998 buffered
      linesFrom(1998, 5), // drops 3
      linesFrom(2003, 10), // drops 10
      linesFrom(2013, 1), // drops 1
      linesFrom(2014, 7), // drops 7
      linesFrom(2021, 0), // empty batch: no-op
    ]);

    expect(state.lines).toHaveLength(2000);
    expect(state.droppedCount).toBe(21);
    expect(ids(state)).toEqual(ids({ lines: linesFrom(21, 2000), droppedCount: 0 }));
  });

  it('keeps only the newest 2,000 of a single 50,000-line replay burst', () => {
    const state = appendLogRingBuffer(EMPTY_LOG_RING_BUFFER, linesFrom(0, 50_000));

    expect(state.lines).toHaveLength(2000);
    expect(state.droppedCount).toBe(48_000);
    expect(state.lines[0]).toEqual(line(48_000));
    expect(state.lines[1999]).toEqual(line(49_999));
  });

  it('counts the lines already buffered when one batch alone fills the cap', () => {
    const before = appendLogRingBuffer(EMPTY_LOG_RING_BUFFER, linesFrom(0, 1500));

    const state = appendLogRingBuffer(before, linesFrom(1500, 50_000));

    // 1,500 buffered + (50,000 - 2,000) from the batch.
    expect(state.droppedCount).toBe(49_500);
    expect(state.lines).toHaveLength(2000);
    expect(ids(state)).toEqual(ids({ lines: linesFrom(49_500, 2000), droppedCount: 0 }));
  });

  it('replaces every buffered line when a batch is exactly the cap', () => {
    const before = appendLogRingBuffer(EMPTY_LOG_RING_BUFFER, linesFrom(0, 500));

    const state = appendLogRingBuffer(before, linesFrom(500, CONSOLE_MAX_LINES));

    expect(state.droppedCount).toBe(500);
    expect(ids(state)).toEqual(ids({ lines: linesFrom(500, 2000), droppedCount: 0 }));
  });

  it('adds pre-dropped lines to the count without changing which lines are kept', () => {
    const state = appendLogRingBuffer(EMPTY_LOG_RING_BUFFER, linesFrom(0, 50_000), CONSOLE_MAX_LINES, 2_500);

    expect(state.droppedCount).toBe(50_500);
    expect(state.lines).toHaveLength(2000);
    expect(state.lines[0]).toEqual(line(48_000));

    const trimmed = appendLogRingBuffer(EMPTY_LOG_RING_BUFFER, linesFrom(0, 3), CONSOLE_MAX_LINES, 4);
    expect(trimmed.droppedCount).toBe(4);
    expect(ids(trimmed)).toEqual([0, 1, 2]);
  });

  it('records pre-dropped lines even when the batch itself is empty', () => {
    const state = appendLogRingBuffer(EMPTY_LOG_RING_BUFFER, [], CONSOLE_MAX_LINES, 7);

    expect(state).not.toBe(EMPTY_LOG_RING_BUFFER);
    expect(state.lines).toEqual([]);
    expect(state.droppedCount).toBe(7);
  });

  it('honours a custom cap passed in place of the default', () => {
    const state = appendLogRingBuffer(EMPTY_LOG_RING_BUFFER, linesFrom(0, 10), 4);

    expect(ids(state)).toEqual([6, 7, 8, 9]);
    expect(state.droppedCount).toBe(6);
  });

  it('does not mutate the previous state or the incoming batch', () => {
    const previousLines = linesFrom(0, 3);
    const incoming = linesFrom(3, 2);
    const previous: LogRingBufferState<Line> = { lines: previousLines, droppedCount: 5 };

    const state = appendLogRingBuffer(previous, incoming, 4);

    expect(previous.lines).toBe(previousLines);
    expect(previousLines).toHaveLength(3);
    expect(previous.droppedCount).toBe(5);
    expect(incoming).toEqual([line(3), line(4)]);
    expect(state).not.toBe(previous);
    expect(state.lines).not.toBe(previousLines);
    expect(state.lines).not.toBe(incoming);
  });
});

describe('EMPTY_LOG_RING_BUFFER', () => {
  it('is an empty, nothing-dropped state', () => {
    expect(EMPTY_LOG_RING_BUFFER.lines).toEqual([]);
    expect(EMPTY_LOG_RING_BUFFER.droppedCount).toBe(0);
  });
});
