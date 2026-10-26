/**
 * `launch-diagnostic` and `launch-complete` payloads now carry an optional
 * `session_id`. useLaunchState must still match diagnostics by `log_tail_path`
 * and ignore the extra key.
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
        case 'check_game_running':
          return false;
        case 'validate_launch':
          return null;
        case 'launch_game':
          return {
            succeeded: true,
            message: 'Game launch started.',
            helper_log_path: HELPER_LOG_PATH,
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
});
