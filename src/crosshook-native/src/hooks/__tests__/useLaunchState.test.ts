/**
 * Focused tests for useLaunchState's resetLaunchSession path.
 *
 * resetLaunchSession must best-effort cancel backend launch sessions via
 * `launch_reset_sessions` (keyed by the current profile name) and always
 * reset local launch state — even when the backend command rejects.
 */
import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { makeLaunchRequest } from '@/test/fixtures';
import type { OfflineReadinessReport } from '@/types';
import { LaunchPhase } from '@/types';
import { useLaunchState } from '../useLaunchState';

const callCommandMock = vi.fn();

vi.mock('@/lib/ipc', () => ({
  callCommand: (name: string, args?: unknown) => callCommandMock(name, args),
}));

vi.mock('@/lib/events', () => ({
  subscribeEvent: vi.fn((): Promise<() => void> => Promise.resolve(() => {})),
}));

const readinessReport: OfflineReadinessReport = {
  profile_name: 'Synthetic Quest',
  score: 100,
  readiness_state: 'ready',
  trainer_type: 'exe',
  checks: [],
  blocking_reasons: [],
  checked_at: '2026-07-02T12:00:00.000Z',
};

function mockCommands(overrides: Record<string, () => Promise<unknown>> = {}): void {
  callCommandMock.mockImplementation(async (name: string) => {
    const override = overrides[name];
    if (override) {
      return override();
    }
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
          helper_log_path: '/tmp/game.log',
          warnings: [],
        };
      case 'launch_reset_sessions':
        return 1;
      default:
        throw new Error(`unexpected command: ${name}`);
    }
  });
}

function renderLaunchState() {
  return renderHook(() =>
    useLaunchState({
      profileId: 'Synthetic Quest',
      profileName: 'Synthetic Quest',
      method: 'proton_run',
      request: makeLaunchRequest(),
    })
  );
}

describe('useLaunchState resetLaunchSession', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockCommands();
  });

  it('invokes launch_reset_sessions with the profile name and resets local state', async () => {
    const { result } = renderLaunchState();

    await act(async () => {
      await result.current.launchGame();
    });
    expect(result.current.phase).toBe(LaunchPhase.WaitingForTrainer);

    await act(async () => {
      await result.current.resetLaunchSession();
    });

    expect(callCommandMock).toHaveBeenCalledWith('launch_reset_sessions', { profileName: 'Synthetic Quest' });
    expect(result.current.phase).toBe(LaunchPhase.Idle);
  });

  it('still resets local state when the backend reset command rejects', async () => {
    mockCommands({
      launch_reset_sessions: () => Promise.reject(new Error('registry unavailable')),
    });
    const { result } = renderLaunchState();

    await act(async () => {
      await result.current.launchGame();
    });
    expect(result.current.phase).toBe(LaunchPhase.WaitingForTrainer);

    await act(async () => {
      await result.current.resetLaunchSession();
    });

    expect(callCommandMock).toHaveBeenCalledWith('launch_reset_sessions', { profileName: 'Synthetic Quest' });
    expect(result.current.phase).toBe(LaunchPhase.Idle);
  });

  it('does not resurrect launch state when resetLaunchSession resolves before an in-flight launchGame', async () => {
    let resolveLaunchGame: (value: unknown) => void = () => {};
    const pendingLaunchGame = new Promise((resolve) => {
      resolveLaunchGame = resolve;
    });
    mockCommands({
      launch_game: () => pendingLaunchGame,
      launch_reset_sessions: () => Promise.resolve(0),
    });
    const { result } = renderLaunchState();

    let launchGamePromise: Promise<void> = Promise.resolve();
    act(() => {
      launchGamePromise = result.current.launchGame();
    });

    // Wait until launchGame has actually reached the pending backend call.
    await waitFor(() => {
      expect(callCommandMock).toHaveBeenCalledWith('launch_game', expect.anything());
    });

    await act(async () => {
      await result.current.resetLaunchSession();
    });
    expect(result.current.phase).toBe(LaunchPhase.Idle);

    await act(async () => {
      resolveLaunchGame({
        succeeded: true,
        message: 'Game launch started.',
        helper_log_path: '/tmp/game.log',
        warnings: [],
      });
      await launchGamePromise;
    });

    // The reset invalidated launchGame's generation — its late success must
    // not resurrect WaitingForTrainer/SessionActive.
    expect(result.current.phase).toBe(LaunchPhase.Idle);
  });
});
