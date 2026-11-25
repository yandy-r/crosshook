import { act, fireEvent, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { emitMockEvent } from '@/lib/events';
import { renderWithMocks } from '@/test/render';
import { CONSOLE_MAX_LINES } from '@/utils/logRingBuffer';
import { ConsoleView } from '../ConsoleView';

/** Lets the asynchronous `subscribeEvent` promises resolve before any event is emitted. */
async function settleSubscription(): Promise<void> {
  await act(async () => {
    await Promise.resolve();
  });
}

/** `src/test/setup.ts` implements `requestAnimationFrame` as `setTimeout(cb, 16)`. */
async function flushAnimationFrame(): Promise<void> {
  await act(async () => {
    await new Promise<void>((resolve) => {
      window.setTimeout(resolve, 40);
    });
  });
}

function emitBurst(count: number, firstIndex = 0): void {
  for (let index = 0; index < count; index += 1) {
    emitMockEvent('launch-log', `line ${firstIndex + index}`);
  }
}

function lineNodes(): HTMLElement[] {
  return Array.from(document.querySelectorAll<HTMLElement>('.crosshook-console__line'));
}

function lineTexts(): string[] {
  return lineNodes().map((node) => node.querySelector('.crosshook-console__code')?.textContent ?? '');
}

function droppedIndicator(): HTMLElement | null {
  return document.querySelector<HTMLElement>('.crosshook-console__dropped');
}

describe('ConsoleView', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('renders the line of an object launch-log payload carrying a session_id', async () => {
    renderWithMocks(<ConsoleView />);
    await settleSubscription();

    act(() => {
      emitMockEvent('launch-log', { line: 'game process started', session_id: 's1' });
    });
    await flushAnimationFrame();

    expect(screen.getByText('game process started')).toBeInTheDocument();
    expect(screen.queryByText(/\{"line"/)).not.toBeInTheDocument();
    expect(screen.queryByText(/session_id/)).not.toBeInTheDocument();
  });

  it('streams update-log output as well as launch-log output', async () => {
    renderWithMocks(<ConsoleView />);
    await settleSubscription();

    act(() => {
      emitMockEvent('update-log', 'update finished');
    });
    await flushAnimationFrame();

    expect(screen.getByText('update finished')).toBeInTheDocument();
  });

  it('ignores blank payloads without scheduling a frame', async () => {
    const rafSpy = vi.spyOn(window, 'requestAnimationFrame');
    renderWithMocks(<ConsoleView />);
    await settleSubscription();

    act(() => {
      emitMockEvent('launch-log', '  \n\t ');
      emitMockEvent('update-log', '');
      emitMockEvent('launch-log', { line: '   ', session_id: 's1' });
    });
    await flushAnimationFrame();

    expect(rafSpy).not.toHaveBeenCalled();
    expect(lineNodes()).toHaveLength(0);
    expect(screen.getByText('Waiting for log output')).toBeInTheDocument();
  });

  it('batches lines emitted in one tick into a single animation frame', async () => {
    const rafSpy = vi.spyOn(window, 'requestAnimationFrame');
    renderWithMocks(<ConsoleView />);
    await settleSubscription();

    act(() => {
      emitBurst(50);
    });

    expect(rafSpy).toHaveBeenCalledTimes(1);
    expect(lineNodes()).toHaveLength(0);

    await flushAnimationFrame();

    expect(rafSpy).toHaveBeenCalledTimes(1);
    expect(lineTexts()).toEqual(Array.from({ length: 50 }, (_, index) => `line ${index}`));

    // A flushed frame clears its handle, so the next batch gets a frame of its own.
    act(() => {
      emitBurst(50, 50);
    });
    expect(rafSpy).toHaveBeenCalledTimes(2);
    await flushAnimationFrame();

    expect(lineNodes()).toHaveLength(100);
    expect(lineTexts()[99]).toBe('line 99');
    expect(droppedIndicator()).toBeNull();
  });

  it('caps the view at 2,000 lines and reports how many were dropped', async () => {
    renderWithMocks(<ConsoleView />);
    await settleSubscription();

    act(() => {
      emitBurst(2500);
    });
    await flushAnimationFrame();

    expect(lineNodes()).toHaveLength(CONSOLE_MAX_LINES);
    expect(lineTexts()[0]).toBe('line 500');
    expect(lineTexts()[1999]).toBe('line 2499');
    expect(screen.queryByText('line 499')).not.toBeInTheDocument();
    expect(droppedIndicator()).toHaveTextContent('500 earlier lines dropped');
    expect(droppedIndicator()).toHaveAttribute('role', 'note');
    expect(screen.queryByText('Waiting for log output')).not.toBeInTheDocument();
  });

  // The slowest case in the suite by design: 50,000 events (each paying for its own timestamp) and
  // 2,000 rendered nodes. It takes ~1.5 s locally, so allow room on a loaded CI runner.
  it('stays bounded through a 50,000-line replay burst delivered in one frame', async () => {
    renderWithMocks(<ConsoleView />);
    await settleSubscription();

    act(() => {
      emitBurst(50_000);
    });
    await flushAnimationFrame();

    expect(lineNodes()).toHaveLength(CONSOLE_MAX_LINES);
    expect(lineTexts()[0]).toBe('line 48000');
    expect(lineTexts()[1999]).toBe('line 49999');
    expect(droppedIndicator()).toHaveTextContent('48000 earlier lines dropped');
  }, 20_000);

  it('keeps the dropped count exact as later batches roll older lines out', async () => {
    renderWithMocks(<ConsoleView />);
    await settleSubscription();

    act(() => {
      emitBurst(1999);
    });
    await flushAnimationFrame();
    expect(droppedIndicator()).toBeNull();

    act(() => {
      emitBurst(2, 1999);
    });
    await flushAnimationFrame();
    expect(droppedIndicator()).toHaveTextContent('1 earlier line dropped');

    act(() => {
      emitBurst(10, 2001);
    });
    await flushAnimationFrame();
    expect(droppedIndicator()).toHaveTextContent('11 earlier lines dropped');
    expect(lineTexts()[0]).toBe('line 11');
  });

  it('clears rendered lines and the drop indicator, and cancels lines still waiting for a frame', async () => {
    const rafSpy = vi.spyOn(window, 'requestAnimationFrame');
    const cancelSpy = vi.spyOn(window, 'cancelAnimationFrame');
    renderWithMocks(<ConsoleView />);
    await settleSubscription();

    act(() => {
      emitBurst(2500);
    });
    await flushAnimationFrame();
    expect(droppedIndicator()).toHaveTextContent('500 earlier lines dropped');

    act(() => {
      emitMockEvent('launch-log', 'queued but never flushed');
    });
    const queuedFrameId = rafSpy.mock.results.at(-1)?.value as number;

    fireEvent.click(screen.getByRole('button', { name: 'Clear' }));

    expect(cancelSpy).toHaveBeenCalledWith(queuedFrameId);
    expect(lineNodes()).toHaveLength(0);
    expect(droppedIndicator()).toBeNull();
    expect(screen.getByText('Waiting for log output')).toBeInTheDocument();

    await flushAnimationFrame();

    expect(screen.queryByText('queued but never flushed')).not.toBeInTheDocument();
    expect(screen.getByText('Waiting for log output')).toBeInTheDocument();

    act(() => {
      emitMockEvent('launch-log', 'after clear');
    });
    await flushAnimationFrame();

    expect(lineTexts()).toEqual(['after clear']);
    expect(droppedIndicator()).toBeNull();
  });

  it('cancels a queued frame when unmounted', async () => {
    const rafSpy = vi.spyOn(window, 'requestAnimationFrame');
    const cancelSpy = vi.spyOn(window, 'cancelAnimationFrame');
    const { unmount } = renderWithMocks(<ConsoleView />);
    await settleSubscription();

    act(() => {
      emitMockEvent('launch-log', 'queued before unmount');
    });
    const queuedFrameId = rafSpy.mock.results.at(-1)?.value as number;

    unmount();
    await flushAnimationFrame();

    expect(cancelSpy).toHaveBeenCalledWith(queuedFrameId);
    expect(document.querySelector('.crosshook-console')).toBeNull();
  });

  it('follows new flushed batches to the bottom until the user scrolls up', async () => {
    renderWithMocks(<ConsoleView />);
    await settleSubscription();

    const body = screen.getByText('Waiting for log output').closest('.crosshook-console__body') as HTMLDivElement;
    let scrollTop = 0;
    const scrollTopWrites: number[] = [];
    Object.defineProperty(body, 'scrollHeight', { configurable: true, get: () => 1000 });
    Object.defineProperty(body, 'clientHeight', { configurable: true, get: () => 200 });
    Object.defineProperty(body, 'scrollTop', {
      configurable: true,
      get: () => scrollTop,
      set: (value: number) => {
        scrollTop = value;
        scrollTopWrites.push(value);
      },
    });

    act(() => {
      emitMockEvent('launch-log', 'first');
    });
    expect(scrollTopWrites).toEqual([]);

    await flushAnimationFrame();
    expect(scrollTopWrites).toEqual([1000]);

    // The user scrolls away from the bottom: 1000 - (0 + 200) is well past the 24px threshold.
    scrollTop = 0;
    scrollTopWrites.length = 0;
    fireEvent.scroll(body);

    act(() => {
      emitMockEvent('launch-log', 'second');
    });
    await flushAnimationFrame();

    expect(screen.getByText('second')).toBeInTheDocument();
    expect(scrollTopWrites).toEqual([]);
  });

  it('buffers lines while renderLines is false and renders them once expanded', async () => {
    const { rerender } = renderWithMocks(<ConsoleView renderLines={false} />);
    await settleSubscription();

    act(() => {
      emitBurst(5);
    });
    await flushAnimationFrame();

    expect(document.querySelector('.crosshook-console__body')).not.toBeNull();
    expect(lineNodes()).toHaveLength(0);

    rerender(<ConsoleView renderLines />);

    expect(lineTexts()).toEqual(['line 0', 'line 1', 'line 2', 'line 3', 'line 4']);
  });
});
