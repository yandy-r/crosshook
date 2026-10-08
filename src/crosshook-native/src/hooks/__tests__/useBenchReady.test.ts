import { act, renderHook } from '@testing-library/react';
import { StrictMode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const callCommandMock = vi.fn<(name: string) => Promise<void>>();

vi.mock('@/lib/ipc', () => ({
  callCommand: (name: string) => callCommandMock(name),
}));

// The hook uses module-level one-shot flags; import it fresh for every test.
async function importHook() {
  return (await import('../useBenchReady')).useBenchReady;
}

const benchReadyCalls = () => callCommandMock.mock.calls.filter(([name]) => name === 'bench_ready');

describe('useBenchReady', () => {
  beforeEach(() => {
    vi.resetModules();
    vi.useFakeTimers();
    callCommandMock.mockReset().mockResolvedValue(undefined);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('does not fire before ready', async () => {
    const useBenchReady = await importHook();
    renderHook(() => useBenchReady(false));
    act(() => {
      vi.advanceTimersByTime(1000);
    });
    expect(benchReadyCalls()).toHaveLength(0);
  });

  it('fires once after two animation frames once ready', async () => {
    const useBenchReady = await importHook();
    const { rerender } = renderHook(({ ready }) => useBenchReady(ready), { initialProps: { ready: false } });
    act(() => {
      vi.advanceTimersByTime(1000);
    });
    expect(benchReadyCalls()).toHaveLength(0);

    rerender({ ready: true });
    act(() => {
      vi.advanceTimersByTime(16); // first rAF only
    });
    expect(benchReadyCalls()).toHaveLength(0);
    await act(async () => {
      vi.advanceTimersByTime(16); // second rAF fires the call
    });
    expect(benchReadyCalls()).toHaveLength(1);
  });

  it('does not fire again on remount or StrictMode double-invoke', async () => {
    const useBenchReady = await importHook();
    const first = renderHook(() => useBenchReady(true), { wrapper: StrictMode });
    await act(async () => {
      vi.advanceTimersByTime(1000);
    });
    first.unmount();
    const second = renderHook(() => useBenchReady(true), { wrapper: StrictMode });
    await act(async () => {
      vi.advanceTimersByTime(1000);
    });
    second.unmount();
    expect(benchReadyCalls()).toHaveLength(1);
  });

  it('unmount before frames cancel without consuming the one-shot', async () => {
    const useBenchReady = await importHook();
    const first = renderHook(() => useBenchReady(true));
    first.unmount(); // cleanup cancels pending rAFs before they run
    act(() => {
      vi.advanceTimersByTime(1000);
    });
    expect(benchReadyCalls()).toHaveLength(0);

    // One-shot survived the cancelled mount: a later mount still fires.
    renderHook(() => useBenchReady(true));
    await act(async () => {
      vi.advanceTimersByTime(1000);
    });
    expect(benchReadyCalls()).toHaveLength(1);
  });

  it('warns once when the command rejects', async () => {
    callCommandMock.mockRejectedValue(new Error('boom'));
    const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const useBenchReady = await importHook();
    renderHook(() => useBenchReady(true));
    await act(async () => {
      vi.advanceTimersByTime(1000);
    });
    expect(benchReadyCalls()).toHaveLength(1);
    await act(async () => {}); // flush rejection microtask
    expect(warnSpy).toHaveBeenCalledTimes(1);
  });
});
