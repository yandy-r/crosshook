/**
 * The running-sessions store must subscribe before it snapshots, reconcile the
 * snapshot with buffered events by seq, share one subscription across
 * consumers, and never poll.
 */
import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { LaunchSessionChangedPayload, RunningSession, RunningSessionsSnapshot } from '@/types/launch-sessions';
import { useRunningSessions } from '../runningSessionsStore';
import { useRunningProfiles } from '../useRunningProfiles';

type EventHandler = (event: { event: string; id: number; payload: unknown }) => void;

const order: string[] = [];
const eventHandlers = new Set<EventHandler>();
const unlistenSpy = vi.fn();
let subscriptionGate: PromiseWithResolvers<void> | null = null;
let snapshotRequests: PromiseWithResolvers<RunningSessionsSnapshot>[] = [];

const subscribeEventMock = vi.fn(async (name: string, handler: EventHandler) => {
  order.push(`subscribe:${name}`);
  await subscriptionGate?.promise;
  eventHandlers.add(handler);
  order.push(`subscribed:${name}`);
  return () => {
    eventHandlers.delete(handler);
    unlistenSpy();
  };
});

const callCommandMock = vi.fn((name: string) => {
  order.push(`command:${name}`);
  if (name !== 'list_running_sessions') {
    return Promise.reject(new Error(`unexpected command: ${name}`));
  }
  const request = Promise.withResolvers<RunningSessionsSnapshot>();
  snapshotRequests.push(request);
  return request.promise;
});

vi.mock('@/lib/events', () => ({
  subscribeEvent: (name: string, handler: EventHandler) => subscribeEventMock(name, handler),
}));

vi.mock('@/lib/ipc', () => ({
  callCommand: (name: string) => callCommandMock(name),
}));

function emit(payload: LaunchSessionChangedPayload): void {
  for (const handler of eventHandlers) {
    handler({ event: 'launch-session-changed', id: 0, payload });
  }
}

function game(sessionId: string, profileName: string | null): RunningSession {
  return { session_id: sessionId, profile_name: profileName, session_kind: 'game' };
}

async function resolveSnapshot(index: number, snapshot: RunningSessionsSnapshot): Promise<void> {
  await waitFor(() => expect(snapshotRequests.length).toBeGreaterThan(index));
  await act(async () => {
    snapshotRequests[index].resolve(snapshot);
    await snapshotRequests[index].promise;
  });
}

describe('runningSessionsStore', () => {
  beforeEach(() => {
    order.length = 0;
    eventHandlers.clear();
    subscriptionGate = null;
    snapshotRequests = [];
  });

  it('awaits the event subscription before requesting the snapshot', async () => {
    subscriptionGate = Promise.withResolvers<void>();
    renderHook(() => useRunningProfiles());

    await waitFor(() => expect(order).toEqual(['subscribe:launch-session-changed']));
    expect(callCommandMock).not.toHaveBeenCalled();

    await act(async () => {
      subscriptionGate?.resolve();
    });

    await waitFor(() =>
      expect(order).toEqual([
        'subscribe:launch-session-changed',
        'subscribed:launch-session-changed',
        'command:list_running_sessions',
      ])
    );
  });

  it('applies buffered events only past the snapshot seq and drops stale ones afterwards', async () => {
    const { result } = renderHook(() => useRunningProfiles());
    await waitFor(() => expect(snapshotRequests).toHaveLength(1));

    // Arrive while the snapshot is pending. seq 3 is covered by the snapshot,
    // which no longer lists Gamma (its exit is folded in), so replaying it would
    // resurrect Gamma; seq 5 is newer and must be applied exactly once.
    act(() => {
      emit({ kind: 'started', ...game('c', 'Gamma'), seq: 3 });
      emit({ kind: 'started', ...game('b', 'Beta'), seq: 5 });
      emit({ kind: 'started', ...game('b', 'Beta'), seq: 5 });
    });
    await resolveSnapshot(0, { sessions: [game('a', 'Alpha')], seq: 4 });
    expect([...result.current].sort()).toEqual(['Alpha', 'Beta']);

    act(() => {
      // Replayed / pre-snapshot seqs are ignored once the baseline is set.
      emit({ kind: 'started', ...game('c', 'Gamma'), seq: 4 });
      emit({ kind: 'exited', ...game('a', 'Alpha'), code: 0, signal: null, seq: 6 });
    });
    expect([...result.current]).toEqual(['Beta']);
  });

  it('re-fetches and replaces state on resync; a failed snapshot keeps state and retries on the next event', async () => {
    const { result } = renderHook(() => useRunningProfiles());
    await resolveSnapshot(0, { sessions: [game('a', 'Alpha')], seq: 2 });
    expect([...result.current]).toEqual(['Alpha']);

    act(() => {
      emit({ kind: 'resync' });
    });
    await resolveSnapshot(1, { sessions: [game('b', 'Beta')], seq: 9 });
    expect([...result.current]).toEqual(['Beta']);
    expect(callCommandMock).toHaveBeenCalledTimes(2);

    act(() => {
      emit({ kind: 'resync' });
    });
    await waitFor(() => expect(snapshotRequests).toHaveLength(3));
    await act(async () => {
      snapshotRequests[2].reject(new Error('ipc down'));
      await snapshotRequests[2].promise.catch(() => undefined);
    });
    // Previous state survives the failure and nothing retries on its own.
    expect([...result.current]).toEqual(['Beta']);
    expect(snapshotRequests).toHaveLength(3);

    // The next event is buffered and triggers the retry; it applies past the new baseline.
    act(() => {
      emit({ kind: 'started', ...game('c', 'Gamma'), seq: 10 });
    });
    expect(snapshotRequests).toHaveLength(4);
    await resolveSnapshot(3, { sessions: [game('b', 'Beta')], seq: 9 });
    expect([...result.current].sort()).toEqual(['Beta', 'Gamma']);
  });

  it('discards a subscription that resolves after its consumer unmounted and remounted', async () => {
    subscriptionGate = Promise.withResolvers<void>();
    const first = renderHook(() => useRunningProfiles());
    await waitFor(() => expect(subscribeEventMock).toHaveBeenCalledTimes(1));
    first.unmount();
    const second = renderHook(() => useRunningProfiles());
    await waitFor(() => expect(subscribeEventMock).toHaveBeenCalledTimes(2));

    await act(async () => {
      subscriptionGate?.resolve();
    });

    // The stale lifecycle's subscription is torn down; only the live one snapshots.
    await waitFor(() => expect(snapshotRequests).toHaveLength(1));
    expect(unlistenSpy).toHaveBeenCalledTimes(1);
    expect(eventHandlers.size).toBe(1);

    await resolveSnapshot(0, { sessions: [game('a', 'Alpha')], seq: 1 });
    act(() => {
      emit({ kind: 'started', ...game('b', 'Beta'), seq: 2 });
    });
    expect([...second.result.current].sort()).toEqual(['Alpha', 'Beta']);
  });

  it('discards a snapshot that resolves after its consumer unmounted and remounted', async () => {
    const first = renderHook(() => useRunningProfiles());
    await waitFor(() => expect(snapshotRequests).toHaveLength(1));
    first.unmount();
    const second = renderHook(() => useRunningProfiles());
    await waitFor(() => expect(snapshotRequests).toHaveLength(2));

    await resolveSnapshot(0, { sessions: [game('stale', 'Stale')], seq: 5 });
    expect(second.result.current.size).toBe(0);

    await resolveSnapshot(1, { sessions: [game('a', 'Alpha')], seq: 1 });
    // seq 3 is past the live baseline (1) and must not be filtered by the stale seq 5.
    act(() => {
      emit({ kind: 'started', ...game('b', 'Beta'), seq: 3 });
    });
    expect([...second.result.current].sort()).toEqual(['Alpha', 'Beta']);
  });

  it('excludes anonymous and trainer sessions from the running profile names', async () => {
    const { result } = renderHook(() => ({ names: useRunningProfiles(), sessions: useRunningSessions() }));
    await resolveSnapshot(0, {
      sessions: [
        game('anon', null),
        { session_id: 't', profile_name: 'Alpha', session_kind: 'trainer' },
        game('b', 'Beta'),
      ],
      seq: 1,
    });

    expect(result.current.sessions.size).toBe(3);
    expect([...result.current.names]).toEqual(['Beta']);
  });

  it('shares one subscription between consumers and unsubscribes after the last unmounts', async () => {
    const first = renderHook(() => useRunningProfiles());
    const second = renderHook(() => useRunningSessions());
    await resolveSnapshot(0, { sessions: [], seq: 0 });

    expect(subscribeEventMock).toHaveBeenCalledTimes(1);
    expect(callCommandMock).toHaveBeenCalledTimes(1);

    first.unmount();
    expect(unlistenSpy).not.toHaveBeenCalled();
    second.unmount();
    expect(unlistenSpy).toHaveBeenCalledTimes(1);
  });

  it('schedules no timers', async () => {
    vi.useFakeTimers();
    renderHook(() => useRunningProfiles());
    await act(async () => {
      await Promise.resolve();
    });
    expect(subscribeEventMock).toHaveBeenCalledTimes(1);
    expect(vi.getTimerCount()).toBe(0);
  });
});
