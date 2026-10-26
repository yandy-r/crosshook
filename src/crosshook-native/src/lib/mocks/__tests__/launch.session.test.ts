import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { makeLaunchRequest } from '@/test/fixtures';
import { configureMockHandlers, mockCallCommand, resetMockHandlers } from '@/test/render';
import type { LaunchResult } from '@/types';
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
    await mockCallCommand<LaunchResult>('launch_game', { request });

    await vi.advanceTimersByTimeAsync(10_000);

    expect(captured.some((entry) => entry.name === 'launch-complete')).toBe(true);
    const sessionIds = new Set(captured.map((entry) => entry.payload.session_id));
    expect(sessionIds.size).toBe(1);
    expect([...sessionIds][0]).toMatch(/^mock-game-session-/);
  });
});
