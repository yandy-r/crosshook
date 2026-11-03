import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { makeLaunchRequest } from '@/test/fixtures';
import { configureMockHandlers, mockCallCommand, resetMockHandlers } from '@/test/render';
import type { LaunchResult, LaunchSessionChangedPayload, RunningSessionsSnapshot } from '@/types';
import { subscribeEvent } from '../../events';

const EVENT_NAMES = ['launch-log', 'launch-diagnostic', 'launch-complete', 'injection-log'] as const;

type Captured = { name: string; payload: Record<string, unknown> };

async function captureLaunchEvents(): Promise<Captured[]> {
  const captured: Captured[] = [];
  for (const name of EVENT_NAMES) {
    await subscribeEvent<Record<string, unknown>>(name, (event) => {
      captured.push({ name, payload: event.payload });
    });
  }
  return captured;
}

describe('mock launch session ids', () => {
  let captured: Captured[];

  beforeEach(async () => {
    vi.useFakeTimers();
    configureMockHandlers();
    captured = await captureLaunchEvents();
  });

  afterEach(() => {
    resetMockHandlers();
    vi.useRealTimers();
  });

  it('launch_trainer emits log, diagnostic, complete and injection events with one session_id', async () => {
    const request = makeLaunchRequest();
    const result = await mockCallCommand<LaunchResult>('launch_trainer', { request });
    expect(result.succeeded).toBe(true);

    await vi.advanceTimersByTimeAsync(10_000);

    const names = new Set(captured.map((entry) => entry.name));
    for (const name of EVENT_NAMES) {
      expect(names.has(name)).toBe(true);
    }

    const logEntry = captured.find((entry) => entry.name === 'launch-log');
    expect(typeof logEntry?.payload.line).toBe('string');

    const sessionIds = new Set(captured.map((entry) => entry.payload.session_id));
    expect(sessionIds.size).toBe(1);
    const [sessionId] = [...sessionIds];
    expect(typeof sessionId).toBe('string');
    expect(sessionId).toMatch(/^mock-trainer-session-/);

    const complete = captured.find((entry) => entry.name === 'launch-complete');
    expect(complete?.payload).toMatchObject({ code: 0, signal: null, session_id: sessionId });
  });

  it('launch_game emits log, diagnostic and complete events with one session_id', async () => {
    const request = makeLaunchRequest();
    const result = await mockCallCommand<LaunchResult>('launch_game', { request });

    await vi.advanceTimersByTimeAsync(10_000);

    expect(captured.some((entry) => entry.name === 'launch-complete')).toBe(true);
    const sessionIds = new Set(captured.map((entry) => entry.payload.session_id));
    expect(sessionIds.size).toBe(1);
    expect([...sessionIds][0]).toMatch(/^mock-game-session-/);
    expect(result.session_id).toBe([...sessionIds][0]);
  });
});

describe('mock launch-session registry', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    configureMockHandlers();
  });

  afterEach(() => {
    resetMockHandlers();
    vi.useRealTimers();
  });

  it('launch_game emits started, keeps the game past helper completion, and exits on reset', async () => {
    const changes: LaunchSessionChangedPayload[] = [];
    const complete: unknown[] = [];
    await subscribeEvent<LaunchSessionChangedPayload>('launch-session-changed', (event) => {
      changes.push(event.payload);
    });
    await subscribeEvent('launch-complete', (event) => {
      complete.push(event.payload);
    });

    const result = await mockCallCommand<LaunchResult>('launch_game', {
      request: { ...makeLaunchRequest(), profile_name: 'Alpha' },
    });
    expect(changes).toEqual([
      { kind: 'started', session_id: result.session_id, profile_name: 'Alpha', session_kind: 'game', seq: 1 },
    ]);
    expect(await mockCallCommand<RunningSessionsSnapshot>('list_running_sessions')).toEqual({
      sessions: [{ session_id: result.session_id, profile_name: 'Alpha', session_kind: 'game' }],
      seq: 1,
    });

    // Helper completion fires `launch-complete` but the game session stays
    // registered, so WaitingForTrainer does not collapse.
    await vi.advanceTimersByTimeAsync(10_000);
    expect(complete).toHaveLength(1);
    expect(changes).toHaveLength(1);

    expect(await mockCallCommand<number>('launch_reset_sessions', { profileName: 'Alpha' })).toBe(1);
    expect(changes[1]).toMatchObject({ kind: 'exited', session_id: result.session_id, code: null, seq: 2 });

    await mockCallCommand('_mock_set_profile_running', { profileName: 'Beta', running: true });
    expect(changes[2]).toMatchObject({ kind: 'started', profile_name: 'Beta', session_kind: 'game', seq: 3 });
    expect(await mockCallCommand<RunningSessionsSnapshot>('list_running_sessions')).toMatchObject({ seq: 3 });
  });

  it('launch_trainer ends its trainer session at helper completion', async () => {
    const changes: LaunchSessionChangedPayload[] = [];
    await subscribeEvent<LaunchSessionChangedPayload>('launch-session-changed', (event) => {
      changes.push(event.payload);
    });

    const result = await mockCallCommand<LaunchResult>('launch_trainer', {
      request: { ...makeLaunchRequest(), profile_name: 'Alpha' },
    });
    await vi.advanceTimersByTimeAsync(10_000);

    expect(changes.map((change) => change.kind)).toEqual(['started', 'exited']);
    expect(changes[1]).toMatchObject({ session_id: result.session_id, session_kind: 'trainer', code: 0 });
  });
});
