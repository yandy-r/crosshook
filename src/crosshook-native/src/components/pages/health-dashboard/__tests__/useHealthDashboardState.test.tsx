/**
 * `useHealthDashboardState` must not poll `navigator.getGamepads()` itself. The
 * controller Y button ("recheck all profiles") is edge-detected by the shared
 * gamepad-nav loop and delivered through `subscribeGamepadYAction`.
 */
import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { emitGamepadYAction } from '@/hooks/gamepad-nav/events';
import { useGamepadNav } from '@/hooks/useGamepadNav';
import type { EnrichedHealthSummary } from '@/types';
import { useHealthDashboardState } from '../useHealthDashboardState';

type Params = Parameters<typeof useHealthDashboardState>[0];

// The hook only reads `reportForProfile` (offline sort order); keep the test off the
// offline-readiness command and startup-report machinery.
vi.mock('@/hooks/useOfflineReadiness', () => ({
  useOfflineReadiness: () => ({ reportForProfile: () => undefined }),
}));

const EMPTY_SUMMARY: EnrichedHealthSummary = {
  profiles: [],
  healthy_count: 0,
  stale_count: 0,
  broken_count: 0,
  total_count: 0,
  validated_at: '2020-01-01T00:00:00.000Z',
};

function makeParams(overrides: Partial<Params> = {}): Params {
  return {
    summary: EMPTY_SUMMARY,
    loading: false,
    error: null,
    batchValidate: vi.fn(async () => {}),
    cachedSnapshots: {},
    favoriteProfiles: [],
    ...overrides,
  };
}

function makeGamepad(buttons: number[]): Gamepad {
  const pressed = new Set(buttons);
  return {
    axes: [0, 0],
    buttons: Array.from({ length: 16 }, (_, index) => ({
      pressed: pressed.has(index),
      touched: pressed.has(index),
      value: pressed.has(index) ? 1 : 0,
    })),
    connected: true,
    id: 'Mock Gamepad',
    index: 0,
    mapping: 'standard',
    timestamp: Date.now(),
    vibrationActuator: null,
    hapticActuators: [],
  } as unknown as Gamepad;
}

/** One poll frame of the gamepad-nav loop (`src/test/setup.ts` implements rAF as `setTimeout(cb, 16)`). */
function advanceFrame(): void {
  act(() => {
    vi.advanceTimersByTime(20);
  });
}

describe('useHealthDashboardState gamepad recheck', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('mounts without scheduling an animation frame, even while a gamepad holds Y', () => {
    const rafSpy = vi.spyOn(window, 'requestAnimationFrame');
    vi.spyOn(navigator, 'getGamepads').mockReturnValue([makeGamepad([3])] as unknown as Gamepad[]);
    const batchValidate = vi.fn(async () => {});

    renderHook(() => useHealthDashboardState(makeParams({ batchValidate })));
    act(() => {
      vi.advanceTimersByTime(1000);
    });

    expect(rafSpy).not.toHaveBeenCalled();
    expect(batchValidate).not.toHaveBeenCalled();

    // Positive control: the spy does see frames when one is requested.
    window.requestAnimationFrame(() => {});
    expect(rafSpy).toHaveBeenCalledTimes(1);
  });

  it('revalidates when the Y action is emitted', () => {
    const batchValidate = vi.fn(async () => {});
    const { result } = renderHook(() => useHealthDashboardState(makeParams({ batchValidate })));
    expect(batchValidate).not.toHaveBeenCalled();

    act(() => {
      emitGamepadYAction();
    });

    expect(batchValidate).toHaveBeenCalledTimes(1);
    expect(result.current.recheckPendingRef.current).toBe(true);
  });

  it('ignores the Y action while a validation is already running', () => {
    const batchValidate = vi.fn(async () => {});
    const { result } = renderHook(() => useHealthDashboardState(makeParams({ batchValidate, loading: true })));

    act(() => {
      emitGamepadYAction();
    });

    expect(batchValidate).not.toHaveBeenCalled();
    expect(result.current.recheckPendingRef.current).toBe(false);
  });

  it('accepts the Y action again once loading finishes', () => {
    const batchValidate = vi.fn(async () => {});
    const { result, rerender } = renderHook(
      ({ loading }: { loading: boolean }) => useHealthDashboardState(makeParams({ batchValidate, loading })),
      { initialProps: { loading: true } }
    );

    act(() => {
      emitGamepadYAction();
    });
    expect(batchValidate).not.toHaveBeenCalled();

    rerender({ loading: false });
    act(() => {
      emitGamepadYAction();
    });

    expect(batchValidate).toHaveBeenCalledTimes(1);
    expect(result.current.recheckPendingRef.current).toBe(true);
  });

  it('announces the Y-triggered recheck and its result', () => {
    const batchValidate = vi.fn(async () => {});
    const { result, rerender } = renderHook(
      ({ loading, summary }: { loading: boolean; summary: EnrichedHealthSummary | null }) =>
        useHealthDashboardState(makeParams({ batchValidate, loading, summary })),
      { initialProps: { loading: false, summary: null } }
    );
    expect(result.current.ariaAnnouncement).toBe('');

    act(() => {
      emitGamepadYAction();
    });
    rerender({ loading: true, summary: null });
    expect(result.current.ariaAnnouncement).toBe('Checking all profiles...');

    rerender({ loading: false, summary: EMPTY_SUMMARY });
    expect(result.current.ariaAnnouncement).toBe('Validation complete. 0 broken, 0 stale, 0 healthy.');
    expect(result.current.recheckPendingRef.current).toBe(false);
  });

  it('stops reacting to the Y action after unmount', () => {
    const batchValidate = vi.fn(async () => {});
    const { unmount } = renderHook(() => useHealthDashboardState(makeParams({ batchValidate })));

    unmount();
    act(() => {
      emitGamepadYAction();
    });

    expect(batchValidate).not.toHaveBeenCalled();
  });

  it('revalidates when the controller Y button is pressed while controller mode is on', () => {
    let yPressed = false;
    vi.spyOn(navigator, 'getGamepads').mockImplementation(
      () => [makeGamepad(yPressed ? [3] : [])] as unknown as Gamepad[]
    );
    const batchValidate = vi.fn(async () => {});

    renderHook(() => {
      useGamepadNav({ enabled: true });
      return useHealthDashboardState(makeParams({ batchValidate }));
    });

    advanceFrame();
    expect(batchValidate).not.toHaveBeenCalled();

    yPressed = true;
    advanceFrame();
    expect(batchValidate).toHaveBeenCalledTimes(1);

    // Holding the button is not a second press.
    advanceFrame();
    expect(batchValidate).toHaveBeenCalledTimes(1);
  });

  it('ignores the controller Y button while controller mode is off', () => {
    let yPressed = false;
    vi.spyOn(navigator, 'getGamepads').mockImplementation(
      () => [makeGamepad(yPressed ? [3] : [])] as unknown as Gamepad[]
    );
    const batchValidate = vi.fn(async () => {});

    renderHook(() => {
      useGamepadNav({ enabled: false });
      return useHealthDashboardState(makeParams({ batchValidate }));
    });

    advanceFrame();
    yPressed = true;
    advanceFrame();
    advanceFrame();

    expect(batchValidate).not.toHaveBeenCalled();
  });
});
