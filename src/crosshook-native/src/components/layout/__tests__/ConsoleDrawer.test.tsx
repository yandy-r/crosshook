import { act, screen, waitFor } from '@testing-library/react';
import type { PanelImperativeHandle } from 'react-resizable-panels';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { HostReadinessProvider } from '@/context/HostReadinessContext';
import { emitMockEvent } from '@/lib/events';
import { renderWithMocks } from '@/test/render';
import { ConsoleDrawer } from '../ConsoleDrawer';

function createPanelRef() {
  return {
    current: {
      collapse: vi.fn(),
      expand: vi.fn(),
    } as unknown as PanelImperativeHandle,
  };
}

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

describe('ConsoleDrawer', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('renders the compact status bar on narrow layouts', async () => {
    const panelRef = createPanelRef();

    renderWithMocks(
      <HostReadinessProvider>
        <ConsoleDrawer panelRef={panelRef} mode="status" />
      </HostReadinessProvider>
    );

    expect(screen.getByTestId('console-status-bar')).toBeInTheDocument();
    expect(screen.getByText('Runtime console')).toBeInTheDocument();
    expect(screen.getByText('⌘K commands')).toBeInTheDocument();

    await waitFor(() => {
      expect(screen.getByText(/required/i)).toBeInTheDocument();
      expect(screen.getByText(/capabilities/i)).toBeInTheDocument();
    });
  });

  it('keeps the wide drawer collapsed when log events arrive', async () => {
    const panelRef = createPanelRef();

    renderWithMocks(
      <HostReadinessProvider>
        <ConsoleDrawer panelRef={panelRef} mode="drawer" />
      </HostReadinessProvider>
    );

    const toggle = screen.getByRole('button', { name: /runtime console/i });
    expect(toggle).toHaveAttribute('aria-expanded', 'false');
    await settleSubscription();

    act(() => {
      emitMockEvent('launch-log', 'first line\nsecond line');
    });
    await flushAnimationFrame();

    expect(toggle).toHaveAttribute('aria-expanded', 'false');
    expect(screen.getByText('2 lines')).toBeInTheDocument();
    expect(panelRef.current.expand).not.toHaveBeenCalled();
  });

  it('counts an object launch-log payload carrying a session_id as one line', async () => {
    const panelRef = createPanelRef();

    renderWithMocks(
      <HostReadinessProvider>
        <ConsoleDrawer panelRef={panelRef} mode="drawer" />
      </HostReadinessProvider>
    );
    await settleSubscription();

    act(() => {
      emitMockEvent('launch-log', { line: 'a', session_id: 's1' });
    });
    await flushAnimationFrame();

    expect(screen.getByText('1 line')).toBeInTheDocument();
  });

  it('applies the lines of several events received in one tick as a single count update', async () => {
    const panelRef = createPanelRef();
    // The status bar has no ConsoleView inside, so every animation frame belongs to the drawer itself.
    const rafSpy = vi.spyOn(window, 'requestAnimationFrame');

    renderWithMocks(
      <HostReadinessProvider>
        <ConsoleDrawer panelRef={panelRef} mode="status" />
      </HostReadinessProvider>
    );
    await settleSubscription();
    expect(screen.getByText('0 lines')).toBeInTheDocument();

    const framesBefore = rafSpy.mock.calls.length;
    act(() => {
      emitMockEvent('launch-log', 'first line\nsecond line');
      emitMockEvent('update-log', 'third line');
      emitMockEvent('launch-log', 'fourth line\nfifth line\nsixth line');
    });

    // Six lines from three events cost exactly one frame, and nothing is applied before it runs.
    expect(rafSpy.mock.calls.length - framesBefore).toBe(1);
    expect(screen.getByText('0 lines')).toBeInTheDocument();

    await flushAnimationFrame();
    expect(screen.getByText('6 lines')).toBeInTheDocument();
    expect(rafSpy.mock.calls.length - framesBefore).toBe(1);

    act(() => {
      emitMockEvent('launch-log', 'seventh line');
    });
    expect(rafSpy.mock.calls.length - framesBefore).toBe(2);
    await flushAnimationFrame();

    // The count accumulates across frames rather than being replaced by each batch.
    expect(screen.getByText('7 lines')).toBeInTheDocument();
  });

  it('cancels a queued count update when unmounted', async () => {
    const panelRef = createPanelRef();
    const rafSpy = vi.spyOn(window, 'requestAnimationFrame');
    const cancelSpy = vi.spyOn(window, 'cancelAnimationFrame');

    const { unmount } = renderWithMocks(
      <HostReadinessProvider>
        <ConsoleDrawer panelRef={panelRef} mode="status" />
      </HostReadinessProvider>
    );
    await settleSubscription();

    act(() => {
      emitMockEvent('launch-log', 'counted only if flushed');
    });
    const queuedFrameId = rafSpy.mock.results.at(-1)?.value as number;

    unmount();
    await flushAnimationFrame();

    expect(cancelSpy).toHaveBeenCalledWith(queuedFrameId);
    expect(screen.queryByTestId('console-status-bar')).not.toBeInTheDocument();
  });
});
