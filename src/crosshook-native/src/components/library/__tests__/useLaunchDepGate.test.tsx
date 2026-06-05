import { renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { makeProfileDraft } from '@/test/fixtures';
import { useLaunchDepGate } from '../launch/useLaunchDepGate';

type PrefixDepCompleteHandler = (event: {
  payload: {
    profile_name: string;
    prefix_path: string;
    succeeded: boolean;
  };
}) => void;

const launchGameMock = vi.fn();
const launchTrainerMock = vi.fn();
const getDependencyStatusMock = vi.fn();
const installPrefixDependencyMock = vi.fn();
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
    isGamescopeRunning: false,
  }),
}));

vi.mock('@/lib/events', () => ({
  subscribeEvent: (name: string, handler: PrefixDepCompleteHandler) => subscribeEventMock(name, handler),
}));

function emitPrefixDepComplete() {
  for (const handler of eventHandlers) {
    handler({
      payload: {
        profile_name: 'Synthetic Quest',
        prefix_path: '/mock/pfx',
        succeeded: true,
      },
    });
  }
}

describe('useLaunchDepGate', () => {
  beforeEach(() => {
    eventHandlers = [];
    launchGameMock.mockReset();
    launchTrainerMock.mockReset();
    getDependencyStatusMock.mockReset();
    installPrefixDependencyMock.mockReset();
    subscribeEventMock.mockReset();
    subscribeEventMock.mockImplementation((_name: string, handler: PrefixDepCompleteHandler) => {
      eventHandlers.push(handler);
      return Promise.resolve(vi.fn());
    });
  });

  it('silent-catches dependency status failures and allows launch', async () => {
    getDependencyStatusMock.mockRejectedValue(new Error('network error'));

    const { result } = renderHook(() =>
      useLaunchDepGate({
        profile: makeProfileDraft({
          game: { name: 'Synthetic Quest', executable_path: '/games/synthetic.exe' },
          trainer: {
            path: '/trainers/synthetic.exe',
            type: 'exe',
            loading_mode: 'source_directory',
            required_protontricks: ['vcrun2019'],
          },
          runtime: { prefix_path: '/mock/pfx', proton_path: '', working_directory: '' },
        }),
        selectedName: 'Synthetic Quest',
        autoInstallPrefixDeps: false,
      })
    );

    await expect(result.current.handleBeforeLaunch('game')).resolves.toBe(true);
    expect(getDependencyStatusMock).toHaveBeenCalledWith('Synthetic Quest', '/mock/pfx');
    expect(subscribeEventMock).not.toHaveBeenCalled();
  });

  it('ignores prefix-dep-complete while the modal is closed', async () => {
    const { result } = renderHook(() =>
      useLaunchDepGate({
        profile: makeProfileDraft({
          game: { name: 'Synthetic Quest', executable_path: '/games/synthetic.exe' },
          trainer: {
            path: '/trainers/synthetic.exe',
            type: 'exe',
            loading_mode: 'source_directory',
            required_protontricks: ['vcrun2019'],
          },
          runtime: { prefix_path: '/mock/pfx', proton_path: '', working_directory: '' },
        }),
        selectedName: 'Synthetic Quest',
        autoInstallPrefixDeps: false,
      })
    );

    emitPrefixDepComplete();

    await waitFor(() => {
      expect(result.current.depGatePackages).toBeNull();
    });
    expect(subscribeEventMock).not.toHaveBeenCalled();
    expect(launchGameMock).not.toHaveBeenCalled();
    expect(launchTrainerMock).not.toHaveBeenCalled();
  });
});
