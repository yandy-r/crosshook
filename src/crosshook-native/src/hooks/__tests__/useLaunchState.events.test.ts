/**
 * `launch-diagnostic` and `launch-complete` payloads now carry an optional
 * `session_id`. useLaunchState must still match diagnostics by `log_tail_path`
 * and ignore the extra key. Game exit is detected from the hook's own
 * `launch-session-changed` session, not by polling.
 */
import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { makeLaunchRequest } from '@/test/fixtures';
import type { DiagnosticReport, LaunchDiagnosticEvent, OfflineReadinessReport } from '@/types';
import { LaunchPhase } from '@/types';
import { useLaunchState } from '../useLaunchState';

type Handler = (event: { event: string; id: number; payload: unknown }) => void;

const handlers = new Map<string, Handler>();
const callCommandMock = vi.fn();

vi.mock('@/lib/ipc', () => ({
  callCommand: (name: string, args?: unknown) => callCommandMock(name, args),
}));

vi.mock('@/lib/events', () => ({
  subscribeEvent: vi.fn((name: string, handler: Handler): Promise<() => void> => {
    handlers.set(name, handler);
    return Promise.resolve(() => {
      handlers.delete(name);
    });
  }),
}));

const HELPER_LOG_PATH = '/tmp/session-events-game.log';
const OWN_SESSION_ID = 'own-game-session';

const readinessReport: OfflineReadinessReport = {
  profile_name: 'Synthetic Quest',
  score: 100,
  readiness_state: 'ready',
  trainer_type: 'exe',
  checks: [],
  blocking_reasons: [],
  checked_at: '2026-07-02T12:00:00.000Z',
};

function makeReport(logTailPath: string | null): DiagnosticReport {
  return {
    severity: 'warning',
    summary: 'Synthetic diagnostic',
    exit_info: {
      code: 1,
      signal: null,
      signal_name: null,
      core_dumped: false,
      failure_mode: 'non_zero_exit',
      description: 'Process exited with code 1.',
      severity: 'warning',
    },
    pattern_matches: [],
    suggestions: [],
    launch_method: 'proton_run',
    log_tail_path: logTailPath,
    analyzed_at: '2026-07-02T12:00:00.000Z',
  };
}

function emit(name: string, payload: unknown): void {
  const handler = handlers.get(name);
  if (!handler) {
    throw new Error(`no subscriber for ${name}`);
  }
  handler({ event: name, id: 0, payload });
}

async function renderLaunchedHook() {
  const rendered = renderHook(() =>
    useLaunchState({
      profileId: 'Synthetic Quest',
      profileName: 'Synthetic Quest',
      method: 'proton_run',
      request: makeLaunchRequest(),
    })
  );
  await waitFor(() => {
    expect(handlers.has('launch-diagnostic')).toBe(true);
    expect(handlers.has('launch-complete')).toBe(true);
    expect(handlers.has('launch-session-changed')).toBe(true);
  });
  await act(async () => {
    await rendered.result.current.launchGame();
  });
  expect(rendered.result.current.phase).toBe(LaunchPhase.WaitingForTrainer);
  return rendered;
}

describe('useLaunchState session_id events', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    handlers.clear();
    callCommandMock.mockImplementation(async (name: string) => {
      switch (name) {
        case 'check_offline_readiness':
          return readinessReport;
        case 'list_running_sessions':
          return { sessions: [], seq: 0 };
        case 'validate_launch':
          return null;
        case 'launch_trainer':
          return {
            succeeded: true,
            message: 'Trainer launch started.',
            helper_log_path: '/tmp/session-events-trainer.log',
            session_id: 'own-trainer-session',
            warnings: [],
          };
        case 'launch_game':
          return {
            succeeded: true,
            message: 'Game launch started.',
            helper_log_path: HELPER_LOG_PATH,
            session_id: OWN_SESSION_ID,
            warnings: [],
          };
        default:
          throw new Error(`unexpected command: ${name}`);
      }
    });
  });

  it('accepts a launch-diagnostic with an extra session_id key that matches by log_tail_path', async () => {
    const { result } = await renderLaunchedHook();
    const payload: LaunchDiagnosticEvent = { ...makeReport(HELPER_LOG_PATH), session_id: 'session-1' };

    act(() => {
      emit('launch-diagnostic', payload);
    });

    expect(result.current.diagnosticReport).toEqual(payload);
    expect(result.current.diagnosticReport?.summary).toBe('Synthetic diagnostic');
  });

  it('still ignores a launch-diagnostic whose log_tail_path does not match', async () => {
    const { result } = await renderLaunchedHook();

    act(() => {
      emit('launch-diagnostic', { ...makeReport('/tmp/other-session.log'), session_id: 'session-2' });
      emit('launch-diagnostic', { ...makeReport(null), session_id: 'session-3' });
    });

    expect(result.current.diagnosticReport).toBeNull();
  });

  it('keeps the launch phase when launch-complete carries a session_id', async () => {
    const { result } = await renderLaunchedHook();

    act(() => {
      emit('launch-complete', { code: 0, signal: null, session_id: 'session-1' });
    });

    expect(result.current.phase).toBe(LaunchPhase.WaitingForTrainer);
  });

  it('dispatches game-stopped only when its own game session exits, ignoring other sessions', async () => {
    const { result } = await renderLaunchedHook();
    const own = { session_id: OWN_SESSION_ID, profile_name: 'Synthetic Quest', session_kind: 'game' } as const;
    const other = { session_id: 'other-session', profile_name: 'Synthetic Quest', session_kind: 'game' } as const;
    const anonymous = { session_id: 'anon-session', profile_name: null, session_kind: 'game' } as const;

    // An anonymous game session never counts for this profile.
    act(() => {
      emit('launch-session-changed', { kind: 'started', ...anonymous, seq: 1 });
    });
    expect(result.current.isGameRunning).toBe(false);

    // Another game session for the same profile marks the game running, but
    // its exit does not end this hook's tracked launch.
    act(() => {
      emit('launch-session-changed', { kind: 'started', ...other, seq: 2 });
    });
    await waitFor(() => expect(result.current.isGameRunning).toBe(true));

    act(() => {
      emit('launch-session-changed', { kind: 'started', ...own, seq: 3 });
    });
    await waitFor(() => expect(result.current.isGameRunning).toBe(true));

    act(() => {
      emit('launch-session-changed', { kind: 'exited', ...other, code: 0, signal: null, seq: 4 });
    });
    expect(result.current.phase).toBe(LaunchPhase.WaitingForTrainer);
    expect(result.current.isGameRunning).toBe(true);

    // A second same-profile game is still running when the own session exits:
    // the tracked launch ends anyway, while the profile stays "running".
    act(() => {
      emit('launch-session-changed', { kind: 'started', ...other, session_id: 'other-session-2', seq: 5 });
      emit('launch-session-changed', { kind: 'exited', ...own, code: 0, signal: null, seq: 6 });
    });
    await waitFor(() => expect(result.current.phase).toBe(LaunchPhase.Idle));
    expect(result.current.isGameRunning).toBe(true);
    expect(result.current.canLaunchGame).toBe(false);

    act(() => {
      emit('launch-session-changed', {
        kind: 'exited',
        ...other,
        session_id: 'other-session-2',
        code: 0,
        signal: null,
        seq: 7,
      });
    });
    await waitFor(() => expect(result.current.isGameRunning).toBe(false));
    expect(callCommandMock).not.toHaveBeenCalledWith('check_game_running', expect.anything());
  });

  it("keeps the profile's running game after switching profiles away and back", async () => {
    const rendered = renderHook(
      ({ profile }: { profile: string }) =>
        useLaunchState({
          profileId: profile,
          profileName: profile,
          method: 'proton_run',
          request: makeLaunchRequest(),
        }),
      { initialProps: { profile: 'Synthetic Quest' } }
    );
    await waitFor(() => expect(handlers.has('launch-session-changed')).toBe(true));
    await act(async () => {
      await rendered.result.current.launchGame();
    });
    act(() => {
      emit('launch-session-changed', {
        kind: 'started',
        session_id: OWN_SESSION_ID,
        profile_name: 'Synthetic Quest',
        session_kind: 'game',
        seq: 1,
      });
      // Anonymous and other-profile game sessions never count for this profile.
      emit('launch-session-changed', {
        kind: 'started',
        session_id: 'anon',
        profile_name: null,
        session_kind: 'game',
        seq: 2,
      });
      emit('launch-session-changed', {
        kind: 'started',
        session_id: 'other-profile',
        profile_name: 'Other Game',
        session_kind: 'game',
        seq: 3,
      });
    });
    await waitFor(() => expect(rendered.result.current.isGameRunning).toBe(true));

    rendered.rerender({ profile: 'Second Profile' });
    await waitFor(() => expect(rendered.result.current.phase).toBe(LaunchPhase.Idle));
    expect(rendered.result.current.isGameRunning).toBe(false);
    expect(rendered.result.current.canLaunchGame).toBe(true);

    rendered.rerender({ profile: 'Synthetic Quest' });
    await waitFor(() => expect(rendered.result.current.isGameRunning).toBe(true));
    expect(rendered.result.current.phase).toBe(LaunchPhase.Idle);
    expect(rendered.result.current.canLaunchGame).toBe(false);
    expect(rendered.result.current.hintText).toBe('The game is already running. You can launch the trainer directly.');

    act(() => {
      emit('launch-session-changed', {
        kind: 'exited',
        session_id: OWN_SESSION_ID,
        profile_name: 'Synthetic Quest',
        session_kind: 'game',
        code: 0,
        signal: null,
        seq: 4,
      });
    });
    await waitFor(() => expect(rendered.result.current.isGameRunning).toBe(false));
    expect(rendered.result.current.canLaunchGame).toBe(true);
  });

  it("dispatches game-stopped when the profile's game session exits after a reset dropped the tracked id", async () => {
    const { result } = await renderLaunchedHook();
    const own = { session_id: OWN_SESSION_ID, profile_name: 'Synthetic Quest', session_kind: 'game' } as const;
    act(() => {
      emit('launch-session-changed', { kind: 'started', ...own, seq: 1 });
    });
    await waitFor(() => expect(result.current.isGameRunning).toBe(true));

    act(() => {
      result.current.reset();
    });
    expect(result.current.isGameRunning).toBe(true);
    await act(async () => {
      await result.current.launchTrainer();
    });
    expect(result.current.phase).toBe(LaunchPhase.SessionActive);

    act(() => {
      emit('launch-session-changed', { kind: 'exited', ...own, code: 0, signal: null, seq: 2 });
    });
    await waitFor(() => expect(result.current.phase).toBe(LaunchPhase.Idle));
  });
});
