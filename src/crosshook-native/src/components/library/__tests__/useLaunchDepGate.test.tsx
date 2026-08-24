import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { makeProfileDraft } from '@/test/fixtures';
import { useLaunchDepGate } from '../launch/useLaunchDepGate';

type PrefixDepCompleteHandler = (event: {
  payload: {
    profile_name: string;
    prefix_path: string;
    succeeded: boolean;
    exit_code: number | null;
    install_succeeded: boolean;
    install_exit_code: number | null;
    restore_state: 'not_required' | 'succeeded' | 'failed';
    restore_error: string | null;
  };
}) => void;

const launchGameMock = vi.fn();
const launchTrainerMock = vi.fn();
const getDependencyStatusMock = vi.fn();
const installPrefixDependencyMock = vi.fn();
const getPrefixVersionRepairStatusMock = vi.fn();
const repairPrefixWindowsVersionMock = vi.fn();
const subscribeEventMock = vi.fn();
let eventHandlers: PrefixDepCompleteHandler[];

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

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

function emitPrefixDepComplete() {
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
    getPrefixVersionRepairStatusMock.mockReset();
    repairPrefixWindowsVersionMock.mockReset();
    subscribeEventMock.mockReset();
    subscribeEventMock.mockImplementation((_name: string, handler: PrefixDepCompleteHandler) => {
      eventHandlers.push(handler);
      return Promise.resolve(vi.fn());
    });
    getPrefixVersionRepairStatusMock.mockResolvedValue({ required: false, state: null, last_error: null });
    repairPrefixWindowsVersionMock.mockResolvedValue({ required: false, state: null, last_error: null });
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

  it('checks required prefix repair before packages even when no packages are declared', async () => {
    getPrefixVersionRepairStatusMock.mockResolvedValue({
      required: true,
      state: 'pending',
      last_error: null,
    });

    const { result } = renderHook(() =>
      useLaunchDepGate({
        profile: makeProfileDraft({
          game: { name: 'Synthetic Quest', executable_path: '/games/synthetic.exe' },
          trainer: {
            path: '/trainers/synthetic.exe',
            type: 'exe',
            loading_mode: 'source_directory',
            required_protontricks: [],
          },
          runtime: { prefix_path: '/mock/pfx', proton_path: '', working_directory: '' },
        }),
        selectedName: 'Synthetic Quest',
        autoInstallPrefixDeps: false,
      })
    );

    await act(async () => {
      expect(await result.current.handleBeforeLaunch('game')).toBe(false);
    });
    expect(result.current.depGateRepair).toEqual({ required: true, state: 'pending', last_error: null });
    expect(result.current.depGatePendingAction).toBe('game');
    expect(getDependencyStatusMock).not.toHaveBeenCalled();
  });

  it('resumes the remembered launch only after repair and package recheck succeed', async () => {
    let repairRequired = true;
    getPrefixVersionRepairStatusMock.mockImplementation(async () =>
      repairRequired
        ? { required: true, state: 'failed', last_error: 'restore timed out' }
        : { required: false, state: null, last_error: null }
    );
    repairPrefixWindowsVersionMock.mockImplementation(async () => {
      repairRequired = false;
      return { required: false, state: null, last_error: null };
    });
    getDependencyStatusMock.mockResolvedValue([
      {
        package_name: 'vcrun2019',
        state: 'installed',
        checked_at: null,
        installed_at: null,
        last_error: null,
      },
    ]);

    const { result } = renderHook(() =>
      useLaunchDepGate({
        profile: makeProfileDraft({
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

    await act(async () => {
      expect(await result.current.handleBeforeLaunch('game')).toBe(false);
    });
    expect(launchGameMock).not.toHaveBeenCalled();

    await act(async () => {
      await result.current.repairPrefixVersion();
    });

    expect(repairPrefixWindowsVersionMock).toHaveBeenCalledWith('Synthetic Quest', '/mock/pfx');
    expect(getDependencyStatusMock).toHaveBeenCalledWith('Synthetic Quest', '/mock/pfx');
    expect(launchGameMock).toHaveBeenCalledTimes(1);
    expect(result.current.depGateRepair).toBeNull();
    expect(result.current.depGatePendingAction).toBeNull();
  });

  it('keeps launch blocked and preserves the pending action when restoration fails', async () => {
    getDependencyStatusMock.mockResolvedValue([]);
    installPrefixDependencyMock.mockResolvedValue(undefined);

    const { result } = renderHook(() =>
      useLaunchDepGate({
        profile: makeProfileDraft({
          trainer: {
            path: '/trainers/synthetic.exe',
            type: 'exe',
            loading_mode: 'source_directory',
            required_protontricks: ['dotnet48'],
          },
          runtime: { prefix_path: '/mock/pfx', proton_path: '', working_directory: '' },
        }),
        selectedName: 'Synthetic Quest',
        autoInstallPrefixDeps: true,
      })
    );

    await act(async () => {
      expect(await result.current.handleBeforeLaunch('trainer')).toBe(false);
    });

    act(() => {
      for (const handler of eventHandlers) {
        handler({
          payload: {
            profile_name: 'Synthetic Quest',
            prefix_path: '/mock/pfx',
            succeeded: false,
            exit_code: 0,
            install_succeeded: true,
            install_exit_code: 0,
            restore_state: 'failed',
            restore_error: 'restore timed out',
          },
        });
      }
    });

    await waitFor(() => expect(result.current.depGateRepair?.state).toBe('failed'));
    expect(result.current.depGatePendingAction).toBe('trainer');
    expect(result.current.depGatePackages).toBeNull();
    expect(launchTrainerMock).not.toHaveBeenCalled();
  });

  it('does not launch when installation fails after compatibility restoration succeeds', async () => {
    getDependencyStatusMock.mockResolvedValue([]);
    installPrefixDependencyMock.mockResolvedValue(undefined);

    const { result } = renderHook(() =>
      useLaunchDepGate({
        profile: makeProfileDraft({
          trainer: {
            path: '/trainers/synthetic.exe',
            type: 'exe',
            loading_mode: 'source_directory',
            required_protontricks: ['dotnet48'],
          },
          runtime: { prefix_path: '/mock/pfx', proton_path: '', working_directory: '' },
        }),
        selectedName: 'Synthetic Quest',
        autoInstallPrefixDeps: true,
      })
    );

    await act(async () => {
      expect(await result.current.handleBeforeLaunch('game')).toBe(false);
    });
    act(() => {
      for (const handler of eventHandlers) {
        handler({
          payload: {
            profile_name: 'Synthetic Quest',
            prefix_path: '/mock/pfx',
            succeeded: false,
            exit_code: 42,
            install_succeeded: false,
            install_exit_code: 42,
            restore_state: 'succeeded',
            restore_error: null,
          },
        });
      }
    });

    await waitFor(() => expect(result.current.depGatePendingAction).toBeNull());
    expect(launchGameMock).not.toHaveBeenCalled();
  });

  it('resumes the remembered action when completion arrives during the install IPC call', async () => {
    getDependencyStatusMock.mockResolvedValue([]);
    installPrefixDependencyMock.mockImplementation(async () => {
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
          },
        });
      }
    });

    const { result } = renderHook(() =>
      useLaunchDepGate({
        profile: makeProfileDraft({
          trainer: {
            path: '/trainers/synthetic.exe',
            type: 'exe',
            loading_mode: 'source_directory',
            required_protontricks: ['dotnet48'],
          },
          runtime: { prefix_path: '/mock/pfx', proton_path: '', working_directory: '' },
        }),
        selectedName: 'Synthetic Quest',
        autoInstallPrefixDeps: true,
      })
    );

    await act(async () => {
      expect(await result.current.handleBeforeLaunch('trainer')).toBe(false);
    });

    expect(launchTrainerMock).toHaveBeenCalledTimes(1);
    expect(result.current.depGatePendingAction).toBeNull();
  });

  it('uses the Steam compatdata prefix when the runtime prefix is blank', async () => {
    getPrefixVersionRepairStatusMock.mockResolvedValue({ required: true, state: 'pending', last_error: null });

    const { result } = renderHook(() =>
      useLaunchDepGate({
        profile: makeProfileDraft({
          runtime: { prefix_path: '   ' },
          steam: { compatdata_path: '  /steam/compatdata/123/pfx  ' },
        }),
        selectedName: 'Synthetic Quest',
        autoInstallPrefixDeps: false,
      })
    );

    await act(async () => {
      expect(await result.current.handleBeforeLaunch('game')).toBe(false);
    });

    expect(getPrefixVersionRepairStatusMock).toHaveBeenCalledWith('Synthetic Quest', '/steam/compatdata/123/pfx');
  });

  it('keeps a same-session failed repair non-skippable after cancel and lookup failure', async () => {
    getPrefixVersionRepairStatusMock
      .mockResolvedValueOnce({ required: true, state: 'failed', last_error: 'restore timed out' })
      .mockRejectedValueOnce(new Error('repair journal unavailable'));

    const { result } = renderHook(() =>
      useLaunchDepGate({
        profile: makeProfileDraft({ runtime: { prefix_path: '/mock/pfx' } }),
        selectedName: 'Synthetic Quest',
        autoInstallPrefixDeps: false,
      })
    );

    await act(async () => {
      expect(await result.current.handleBeforeLaunch('trainer')).toBe(false);
    });
    act(() => {
      result.current.setDepGateRepair(null);
      result.current.setDepGatePendingAction(null);
    });

    await act(async () => {
      expect(await result.current.handleBeforeLaunch('trainer')).toBe(false);
    });

    expect(result.current.depGateRepair).toEqual({
      required: true,
      state: 'failed',
      last_error: 'restore timed out',
    });
    expect(result.current.depGatePendingAction).toBe('trainer');
    expect(getDependencyStatusMock).not.toHaveBeenCalled();
  });

  it('rechecks the journal before resuming after a successful restore event', async () => {
    let journalRequired = false;
    getPrefixVersionRepairStatusMock.mockImplementation(async () =>
      journalRequired
        ? { required: true, state: 'failed', last_error: 'journal cleanup failed' }
        : { required: false, state: null, last_error: null }
    );
    getDependencyStatusMock.mockResolvedValue([]);
    installPrefixDependencyMock.mockResolvedValue(undefined);

    const { result } = renderHook(() =>
      useLaunchDepGate({
        profile: makeProfileDraft({
          trainer: {
            path: '/trainers/synthetic.exe',
            type: 'exe',
            loading_mode: 'source_directory',
            required_protontricks: ['dotnet48'],
          },
          runtime: { prefix_path: '/mock/pfx' },
        }),
        selectedName: 'Synthetic Quest',
        autoInstallPrefixDeps: true,
      })
    );

    await act(async () => {
      expect(await result.current.handleBeforeLaunch('game')).toBe(false);
    });
    journalRequired = true;
    act(() => {
      emitPrefixDepComplete();
    });

    await waitFor(() => expect(result.current.depGateRepair?.required).toBe(true));
    expect(result.current.depGateRepair?.last_error).toBe('journal cleanup failed');
    expect(result.current.depGatePendingAction).toBe('game');
    expect(launchGameMock).not.toHaveBeenCalled();
  });

  it('keeps the dependency gate busy until post-restore verification completes', async () => {
    const verification = deferred<{ required: boolean; state: null; last_error: null }>();
    getPrefixVersionRepairStatusMock
      .mockResolvedValueOnce({ required: false, state: null, last_error: null })
      .mockReturnValueOnce(verification.promise);
    getDependencyStatusMock.mockResolvedValue([]);
    installPrefixDependencyMock.mockResolvedValue(undefined);

    const { result } = renderHook(() =>
      useLaunchDepGate({
        profile: makeProfileDraft({
          trainer: { required_protontricks: ['dotnet48'] },
          runtime: { prefix_path: '/mock/pfx' },
        }),
        selectedName: 'Synthetic Quest',
        autoInstallPrefixDeps: true,
      })
    );

    await act(async () => {
      expect(await result.current.handleBeforeLaunch('game')).toBe(false);
    });
    act(() => emitPrefixDepComplete());

    expect(result.current.depGateInstalling).toBe(true);
    expect(result.current.depGateVerifying).toBe(true);
    expect(result.current.depGatePackages).toEqual(['dotnet48']);
    expect(launchGameMock).not.toHaveBeenCalled();

    await act(async () => {
      verification.resolve({ required: false, state: null, last_error: null });
      await verification.promise;
    });
    await waitFor(() => expect(launchGameMock).toHaveBeenCalledTimes(1));
    expect(result.current.depGateInstalling).toBe(false);
    expect(result.current.depGateVerifying).toBe(false);
    expect(result.current.depGatePackages).toBeNull();
  });

  it('does not launch or mutate gate state when profile changes during completion verification', async () => {
    const verification = deferred<{ required: boolean; state: null; last_error: null }>();
    getPrefixVersionRepairStatusMock
      .mockResolvedValueOnce({ required: false, state: null, last_error: null })
      .mockReturnValueOnce(verification.promise);
    getDependencyStatusMock.mockResolvedValue([]);
    installPrefixDependencyMock.mockResolvedValue(undefined);
    const profileA = makeProfileDraft({
      trainer: { required_protontricks: ['dotnet48'] },
      runtime: { prefix_path: '/prefix/a' },
    });
    const profileB = makeProfileDraft({ runtime: { prefix_path: '/prefix/b' } });

    const { result, rerender } = renderHook(
      ({ profile, selectedName }) => useLaunchDepGate({ profile, selectedName, autoInstallPrefixDeps: true }),
      { initialProps: { profile: profileA, selectedName: 'Profile A' } }
    );
    await act(async () => {
      expect(await result.current.handleBeforeLaunch('trainer')).toBe(false);
    });
    act(() => {
      for (const handler of eventHandlers) {
        handler({
          payload: {
            profile_name: 'Profile A',
            prefix_path: '/prefix/a',
            succeeded: true,
            exit_code: 0,
            install_succeeded: true,
            install_exit_code: 0,
            restore_state: 'succeeded',
            restore_error: null,
          },
        });
      }
    });

    rerender({ profile: profileB, selectedName: 'Profile B' });
    await act(async () => {
      verification.resolve({ required: false, state: null, last_error: null });
      await verification.promise;
    });

    expect(launchTrainerMock).not.toHaveBeenCalled();
    expect(result.current.depGatePackages).toBeNull();
    expect(result.current.depGateRepair).toBeNull();
  });

  it('does not launch after unmount during completion verification', async () => {
    const verification = deferred<{ required: boolean; state: null; last_error: null }>();
    getPrefixVersionRepairStatusMock
      .mockResolvedValueOnce({ required: false, state: null, last_error: null })
      .mockReturnValueOnce(verification.promise);
    getDependencyStatusMock.mockResolvedValue([]);
    installPrefixDependencyMock.mockResolvedValue(undefined);

    const { result, unmount } = renderHook(() =>
      useLaunchDepGate({
        profile: makeProfileDraft({
          trainer: { required_protontricks: ['dotnet48'] },
          runtime: { prefix_path: '/mock/pfx' },
        }),
        selectedName: 'Synthetic Quest',
        autoInstallPrefixDeps: true,
      })
    );
    await act(async () => {
      expect(await result.current.handleBeforeLaunch('game')).toBe(false);
    });
    act(() => emitPrefixDepComplete());
    unmount();

    verification.resolve({ required: false, state: null, last_error: null });
    await verification.promise;
    await Promise.resolve();

    expect(launchGameMock).not.toHaveBeenCalled();
  });

  it('unsubscribes and skips install when profile changes before subscription resolves', async () => {
    const subscription = deferred<() => void>();
    const unlisten = vi.fn();
    subscribeEventMock.mockReturnValue(subscription.promise);
    getDependencyStatusMock.mockResolvedValue([]);
    const profileA = makeProfileDraft({
      trainer: { required_protontricks: ['dotnet48'] },
      runtime: { prefix_path: '/prefix/a' },
    });
    const profileB = makeProfileDraft({ runtime: { prefix_path: '/prefix/b' } });
    const { result, rerender } = renderHook(
      ({ profile, selectedName }) => useLaunchDepGate({ profile, selectedName, autoInstallPrefixDeps: true }),
      { initialProps: { profile: profileA, selectedName: 'Profile A' } }
    );

    let gatePromise!: Promise<boolean>;
    act(() => {
      gatePromise = result.current.handleBeforeLaunch('game');
    });
    await waitFor(() => expect(subscribeEventMock).toHaveBeenCalled());
    rerender({ profile: profileB, selectedName: 'Profile B' });
    subscription.resolve(unlisten);
    await act(async () => expect(await gatePromise).toBe(false));

    expect(unlisten).toHaveBeenCalledTimes(1);
    expect(installPrefixDependencyMock).not.toHaveBeenCalled();
  });

  it('unsubscribes and skips install when unmounted before subscription resolves', async () => {
    const subscription = deferred<() => void>();
    const unlisten = vi.fn();
    subscribeEventMock.mockReturnValue(subscription.promise);
    getDependencyStatusMock.mockResolvedValue([]);
    const { result, unmount } = renderHook(() =>
      useLaunchDepGate({
        profile: makeProfileDraft({
          trainer: { required_protontricks: ['dotnet48'] },
          runtime: { prefix_path: '/mock/pfx' },
        }),
        selectedName: 'Synthetic Quest',
        autoInstallPrefixDeps: true,
      })
    );

    let gatePromise!: Promise<boolean>;
    act(() => {
      gatePromise = result.current.handleBeforeLaunch('trainer');
    });
    await waitFor(() => expect(subscribeEventMock).toHaveBeenCalled());
    unmount();
    subscription.resolve(unlisten);
    await act(async () => expect(await gatePromise).toBe(false));

    expect(unlisten).toHaveBeenCalledTimes(1);
    expect(installPrefixDependencyMock).not.toHaveBeenCalled();
  });

  it('does not authorize launch when dependency status rejects after a profile change', async () => {
    let rejectDependency!: (error: Error) => void;
    const dependencyStatus = new Promise<never>((_resolve, reject) => {
      rejectDependency = reject;
    });
    getDependencyStatusMock.mockReturnValue(dependencyStatus);
    const profileA = makeProfileDraft({
      trainer: { required_protontricks: ['dotnet48'] },
      runtime: { prefix_path: '/prefix/a' },
    });
    const profileB = makeProfileDraft({ runtime: { prefix_path: '/prefix/b' } });
    const { result, rerender } = renderHook(
      ({ profile, selectedName }) => useLaunchDepGate({ profile, selectedName, autoInstallPrefixDeps: false }),
      { initialProps: { profile: profileA, selectedName: 'Profile A' } }
    );

    const gatePromise = result.current.handleBeforeLaunch('game');
    await waitFor(() => expect(getDependencyStatusMock).toHaveBeenCalled());
    rerender({ profile: profileB, selectedName: 'Profile B' });
    rejectDependency(new Error('dependency cache unavailable'));

    await expect(gatePromise).resolves.toBe(false);
    expect(launchGameMock).not.toHaveBeenCalled();
  });

  it('does not authorize launch when dependency status rejects after unmount', async () => {
    let rejectDependency!: (error: Error) => void;
    const dependencyStatus = new Promise<never>((_resolve, reject) => {
      rejectDependency = reject;
    });
    getDependencyStatusMock.mockReturnValue(dependencyStatus);
    const { result, unmount } = renderHook(() =>
      useLaunchDepGate({
        profile: makeProfileDraft({
          trainer: { required_protontricks: ['dotnet48'] },
          runtime: { prefix_path: '/mock/pfx' },
        }),
        selectedName: 'Synthetic Quest',
        autoInstallPrefixDeps: false,
      })
    );

    const gatePromise = result.current.handleBeforeLaunch('trainer');
    await waitFor(() => expect(getDependencyStatusMock).toHaveBeenCalled());
    unmount();
    rejectDependency(new Error('dependency cache unavailable'));

    await expect(gatePromise).resolves.toBe(false);
    expect(launchTrainerMock).not.toHaveBeenCalled();
  });

  it('ignores an older install rejection without clearing the newer profile gate', async () => {
    let rejectFirstInstall!: (error: Error) => void;
    const firstInstall = new Promise<void>((_resolve, reject) => {
      rejectFirstInstall = reject;
    });
    installPrefixDependencyMock.mockReturnValueOnce(firstInstall).mockResolvedValueOnce(undefined);
    getDependencyStatusMock.mockResolvedValue([]);
    const profileA = makeProfileDraft({
      trainer: { required_protontricks: ['dotnet48'] },
      runtime: { prefix_path: '/prefix/a' },
    });
    const profileB = makeProfileDraft({
      trainer: { required_protontricks: ['dotnet48'] },
      runtime: { prefix_path: '/prefix/b' },
    });
    const { result, rerender } = renderHook(
      ({ profile, selectedName }) => useLaunchDepGate({ profile, selectedName, autoInstallPrefixDeps: true }),
      { initialProps: { profile: profileA, selectedName: 'Profile A' } }
    );

    let firstGate!: Promise<boolean>;
    act(() => {
      firstGate = result.current.handleBeforeLaunch('game');
    });
    await waitFor(() => expect(installPrefixDependencyMock).toHaveBeenCalledTimes(1));

    rerender({ profile: profileB, selectedName: 'Profile B' });
    await act(async () => {
      expect(await result.current.handleBeforeLaunch('trainer')).toBe(false);
    });
    expect(result.current.depGateInstalling).toBe(true);
    expect(result.current.depGatePackages).toEqual(['dotnet48']);
    expect(result.current.depGatePendingAction).toBe('trainer');

    rejectFirstInstall(new Error('old install failed'));
    await act(async () => expect(await firstGate).toBe(false));

    expect(result.current.depGateInstalling).toBe(true);
    expect(result.current.depGatePackages).toEqual(['dotnet48']);
    expect(result.current.depGatePendingAction).toBe('trainer');

    act(() => {
      for (const handler of eventHandlers) {
        handler({
          payload: {
            profile_name: 'Profile B',
            prefix_path: '/prefix/b',
            succeeded: true,
            exit_code: 0,
            install_succeeded: true,
            install_exit_code: 0,
            restore_state: 'not_required',
            restore_error: null,
          },
        });
      }
    });

    await waitFor(() => expect(launchTrainerMock).toHaveBeenCalledTimes(1));
    expect(result.current.depGateInstalling).toBe(false);
    expect(result.current.depGatePendingAction).toBeNull();
  });
});
