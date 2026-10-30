import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { makeProfileDraft } from '@/test/fixtures';
import type { PrefixDepCompletePayload, PrefixVersionRepairStatus } from '@/types/prefix-deps';
import { useLaunchDepGate } from '../launch/useLaunchDepGate';

type PrefixDepCompleteHandler = (event: { payload: PrefixDepCompletePayload }) => void;

const CLEAN_REPAIR_STATUS: PrefixVersionRepairStatus = {
  required: false,
  state: null,
  last_error: null,
  in_progress: false,
  last_recovery: null,
};

const launchGameMock = vi.fn();
const launchTrainerMock = vi.fn();
const getDependencyStatusMock = vi.fn();
const installPrefixDependencyMock = vi.fn();
const getPrefixVersionRepairStatusMock = vi.fn();
const repairPrefixWindowsVersionMock = vi.fn();
const subscribeEventMock = vi.fn();
let eventHandlers: PrefixDepCompleteHandler[];

vi.mock('@/context/LaunchStateContext', () => ({
  useLaunchStateContext: () => ({
    launchGame: launchGameMock,
    launchTrainer: launchTrainerMock,
  }),
}));

vi.mock('@/hooks/useLaunchPrefixDependencyGate', () => ({
  useLaunchPrefixDependencyGate: () => ({
    getDependencyStatus: getDependencyStatusMock,
    installPrefixDependency: installPrefixDependencyMock,
    getPrefixVersionRepairStatus: getPrefixVersionRepairStatusMock,
    repairPrefixWindowsVersion: repairPrefixWindowsVersionMock,
    isGamescopeRunning: false,
  }),
}));

vi.mock('@/lib/events', () => ({
  subscribeEvent: (name: string, handler: PrefixDepCompleteHandler) => subscribeEventMock(name, handler),
}));

function emitPrefixDepComplete(overrides: Partial<PrefixDepCompletePayload> = {}) {
  for (const handler of eventHandlers) {
    handler({
      payload: {
        profile_name: 'Synthetic Quest',
        prefix_path: '/mock/pfx',
        succeeded: true,
        exit_code: 0,
        install_succeeded: true,
        install_exit_code: 0,
        restore_state: 'succeeded',
        restore_error: null,
        ...overrides,
      },
    });
  }
}

describe('useLaunchDepGate prefix busy and pending restore', () => {
  beforeEach(() => {
    eventHandlers = [];
    launchGameMock.mockReset();
    launchTrainerMock.mockReset();
    getDependencyStatusMock.mockReset();
    installPrefixDependencyMock.mockReset();
    getPrefixVersionRepairStatusMock.mockReset();
    repairPrefixWindowsVersionMock.mockReset();
    subscribeEventMock.mockReset();
    subscribeEventMock.mockImplementation((_name: string, handler: PrefixDepCompleteHandler) => {
      eventHandlers.push(handler);
      return Promise.resolve(vi.fn());
    });
    getPrefixVersionRepairStatusMock.mockResolvedValue(CLEAN_REPAIR_STATUS);
    repairPrefixWindowsVersionMock.mockResolvedValue(CLEAN_REPAIR_STATUS);
  });

  it('blocks for repair when the restore outcome is pending, even if the status lookup later fails', async () => {
    getDependencyStatusMock.mockResolvedValue([]);
    installPrefixDependencyMock.mockResolvedValue(undefined);
    const profile = makeProfileDraft({
      trainer: { required_protontricks: ['dotnet48'] },
      runtime: { prefix_path: '/mock/pfx' },
    });
    const { result } = renderHook(() =>
      useLaunchDepGate({ profile, selectedName: 'Synthetic Quest', autoInstallPrefixDeps: true })
    );

    await act(async () => {
      expect(await result.current.handleBeforeLaunch('game')).toBe(false);
    });
    act(() => {
      emitPrefixDepComplete({
        succeeded: false,
        exit_code: null,
        install_succeeded: false,
        install_exit_code: null,
        restore_state: 'pending',
      });
    });

    await waitFor(() => expect(result.current.depGateRepair?.state).toBe('pending'));
    expect(result.current.depGateRepair?.required).toBe(true);
    expect(result.current.depGatePendingAction).toBe('game');

    act(() => {
      result.current.setDepGateRepair(null);
      result.current.setDepGatePendingAction(null);
    });
    getPrefixVersionRepairStatusMock.mockRejectedValue(new Error('repair journal unavailable'));
    await act(async () => {
      expect(await result.current.handleBeforeLaunch('game')).toBe(false);
    });
    expect(result.current.depGateRepair?.state).toBe('pending');
    expect(launchGameMock).not.toHaveBeenCalled();
  });

  it('shows a busy state instead of the repair flow while a job holds the prefix, then retries', async () => {
    getPrefixVersionRepairStatusMock.mockResolvedValue({
      ...CLEAN_REPAIR_STATUS,
      required: true,
      state: 'pending',
      in_progress: true,
    });
    getDependencyStatusMock.mockResolvedValue([]);
    const profile = makeProfileDraft({
      trainer: { required_protontricks: [] },
      runtime: { prefix_path: '/mock/pfx' },
    });
    const { result } = renderHook(() =>
      useLaunchDepGate({ profile, selectedName: 'Synthetic Quest', autoInstallPrefixDeps: false })
    );

    await act(async () => {
      expect(await result.current.handleBeforeLaunch('trainer')).toBe(false);
    });
    expect(result.current.depGateRepair?.in_progress).toBe(true);
    expect(result.current.depGatePendingAction).toBe('trainer');
    expect(repairPrefixWindowsVersionMock).not.toHaveBeenCalled();

    // The job released the prefix and left no repair behind.
    getPrefixVersionRepairStatusMock.mockResolvedValue(CLEAN_REPAIR_STATUS);
    await act(async () => {
      await result.current.retryPendingLaunch();
    });

    expect(launchTrainerMock).toHaveBeenCalledTimes(1);
    expect(result.current.depGateRepair).toBeNull();
    expect(result.current.depGatePendingAction).toBeNull();
  });
});
