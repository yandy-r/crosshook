import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { UpdateGameResult } from '@/types/update';
import { useUpdateGame } from '../useUpdateGame';

type EventHandler = (event: { event: string; id: number; payload: unknown }) => void;

const handlers = new Map<string, Set<EventHandler>>();
const callCommandMock = vi.fn();

vi.mock('@/lib/ipc', () => ({
  callCommand: (name: string, args?: unknown) => callCommandMock(name, args),
}));

vi.mock('@/lib/events', () => ({
  subscribeEvent: vi.fn((name: string, handler: EventHandler): Promise<() => void> => {
    const set = handlers.get(name) ?? new Set<EventHandler>();
    set.add(handler);
    handlers.set(name, set);
    return Promise.resolve(() => {
      set.delete(handler);
    });
  }),
}));

const UPDATE_RESULT: UpdateGameResult = {
  succeeded: true,
  message: 'Update process launched.',
  helper_log_path: '/mock/logs/update-1.log',
  run_id: 'update-1',
};

function emitComplete(payload: number | null): void {
  for (const handler of handlers.get('update-complete') ?? []) {
    handler({ event: 'update-complete', id: 0, payload });
  }
}

function listenerCount(): number {
  return handlers.get('update-complete')?.size ?? 0;
}

/** `update_game` stays pending until the returned resolver is called. */
function deferUpdateGame(): (result: UpdateGameResult) => void {
  const gate = Promise.withResolvers<UpdateGameResult>();
  callCommandMock.mockImplementation((name: string) => {
    if (name === 'update_game') return gate.promise;
    if (name === 'profile_list') return Promise.resolve([]);
    return Promise.resolve(undefined);
  });
  return gate.resolve;
}

async function renderUpdateHook(stages: string[] = []) {
  const rendered = renderHook(() => {
    const state = useUpdateGame();
    stages.push(state.stage);
    return state;
  });
  await waitFor(() => expect(rendered.result.current.isLoadingProfiles).toBe(false));
  return rendered;
}

function cancelCalls(): number {
  return callCommandMock.mock.calls.filter(([name]) => name === 'cancel_update').length;
}

describe('useUpdateGame', () => {
  beforeEach(() => {
    handlers.clear();
    callCommandMock.mockReset();
    callCommandMock.mockImplementation((name: string) =>
      Promise.resolve(name === 'profile_list' ? [] : name === 'update_game' ? UPDATE_RESULT : undefined)
    );
  });

  it('keeps the complete stage when update-complete arrives before update_game resolves', async () => {
    const stages: string[] = [];
    const { result } = await renderUpdateHook(stages);
    const resolveUpdate = deferUpdateGame();

    let startPromise: Promise<void> = Promise.resolve();
    act(() => {
      startPromise = result.current.startUpdate();
    });
    await waitFor(() => expect(callCommandMock).toHaveBeenCalledWith('update_game', expect.anything()));

    act(() => {
      emitComplete(0);
    });
    expect(result.current.stage).toBe('complete');

    await act(async () => {
      resolveUpdate(UPDATE_RESULT);
      await startPromise;
    });

    expect(result.current.stage).toBe('complete');
    expect(stages).not.toContain('running_updater');
    expect(result.current.result).toEqual(UPDATE_RESULT);
    expect(result.current.error).toBeNull();
    expect(result.current.isRunning).toBe(false);
  });

  it('ignores update-complete when no run is active', async () => {
    const { result } = await renderUpdateHook();
    await act(async () => {
      await result.current.startUpdate();
    });
    expect(result.current.stage).toBe('running_updater');

    act(() => {
      emitComplete(0);
    });
    expect(result.current.stage).toBe('complete');

    // A second completion (e.g. another window's updater) after this run ended.
    act(() => {
      emitComplete(1);
    });
    expect(result.current.stage).toBe('complete');
    expect(result.current.error).toBeNull();
  });

  it('ignores update-complete after reset and cancels a start reset while update_game was in flight', async () => {
    const { result } = await renderUpdateHook();
    const resolveUpdate = deferUpdateGame();

    let startPromise: Promise<void> = Promise.resolve();
    act(() => {
      startPromise = result.current.startUpdate();
    });
    await waitFor(() => expect(callCommandMock).toHaveBeenCalledWith('update_game', expect.anything()));

    act(() => {
      result.current.reset();
    });
    const cancelsAfterReset = cancelCalls();

    await act(async () => {
      resolveUpdate(UPDATE_RESULT);
      await startPromise;
    });

    expect(cancelCalls()).toBe(cancelsAfterReset + 1);
    expect(result.current.stage).toBe('idle');
    expect(result.current.result).toBeNull();
    expect(listenerCount()).toBe(0);

    act(() => {
      emitComplete(1);
    });
    expect(result.current.stage).toBe('idle');
    expect(result.current.error).toBeNull();
  });

  it('does not cancel after a reset start whose updater already completed', async () => {
    const { result } = await renderUpdateHook();
    const resolveUpdate = deferUpdateGame();

    let startPromise: Promise<void> = Promise.resolve();
    act(() => {
      startPromise = result.current.startUpdate();
    });
    await waitFor(() => expect(callCommandMock).toHaveBeenCalledWith('update_game', expect.anything()));

    act(() => {
      result.current.reset();
      emitComplete(0);
    });
    const cancelsAfterReset = cancelCalls();

    await act(async () => {
      resolveUpdate(UPDATE_RESULT);
      await startPromise;
    });

    expect(cancelCalls()).toBe(cancelsAfterReset);
    expect(result.current.stage).toBe('idle');
  });
});
