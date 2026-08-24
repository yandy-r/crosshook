import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { callCommand } from '@/lib/ipc';
import { usePrefixDeps } from '../usePrefixDeps';

vi.mock('@/lib/ipc', () => ({
  callCommand: vi.fn(),
}));

const callCommandMock = vi.mocked(callCommand);

describe('usePrefixDeps', () => {
  beforeEach(() => {
    callCommandMock.mockReset();
    callCommandMock.mockImplementation(async (name) => {
      if (name === 'get_dependency_status') return [];
      if (name === 'get_prefix_version_repair_status') {
        return { required: false, state: null, last_error: null };
      }
      throw new Error(`unexpected command: ${name}`);
    });
  });

  it('reload fetches dependency and repair status again', async () => {
    const { result } = renderHook(() => usePrefixDeps('Ghost', '/prefix'));
    await waitFor(() => expect(callCommandMock).toHaveBeenCalledTimes(2));

    await act(async () => {
      await result.current.reload();
    });

    await waitFor(() => expect(callCommandMock).toHaveBeenCalledTimes(4));
  });

  it('repairs the prefix and refreshes both statuses', async () => {
    let repairRequired = true;
    callCommandMock.mockImplementation(async (name) => {
      if (name === 'get_dependency_status') return [];
      if (name === 'get_prefix_version_repair_status') {
        return repairRequired
          ? { required: true, state: 'failed', last_error: 'restore timed out' }
          : { required: false, state: null, last_error: null };
      }
      if (name === 'repair_prefix_windows_version') {
        repairRequired = false;
        return { required: false, state: null, last_error: null };
      }
      throw new Error(`unexpected command: ${name}`);
    });

    const { result } = renderHook(() => usePrefixDeps('Ghost', '/prefix'));
    await waitFor(() => expect(result.current.repairStatus.required).toBe(true));

    await act(async () => {
      await result.current.repairPrefixVersion();
    });

    expect(callCommandMock).toHaveBeenCalledWith('repair_prefix_windows_version', {
      profileName: 'Ghost',
      prefixPath: '/prefix',
    });
    await waitFor(() => expect(result.current.repairStatus.required).toBe(false));
    expect(callCommandMock).toHaveBeenCalledTimes(5);
  });

  it('keeps a required repair visible when dependency status loading fails', async () => {
    callCommandMock.mockImplementation(async (name) => {
      if (name === 'get_dependency_status') throw new Error('dependency cache unavailable');
      if (name === 'get_prefix_version_repair_status') {
        return { required: true, state: 'failed', last_error: 'restore timed out' };
      }
      throw new Error(`unexpected command: ${name}`);
    });

    const { result } = renderHook(() => usePrefixDeps('Ghost', '/prefix'));

    await waitFor(() => expect(result.current.repairStatus.required).toBe(true));
    expect(result.current.error).toContain('dependency cache unavailable');
  });

  it('preserves a known required repair when a later repair-status reload fails', async () => {
    let repairLookupFails = false;
    callCommandMock.mockImplementation(async (name) => {
      if (name === 'get_dependency_status') return [];
      if (name === 'get_prefix_version_repair_status') {
        if (repairLookupFails) throw new Error('repair journal unavailable');
        return { required: true, state: 'pending', last_error: null };
      }
      throw new Error(`unexpected command: ${name}`);
    });

    const { result } = renderHook(() => usePrefixDeps('Ghost', '/prefix'));
    await waitFor(() => expect(result.current.repairStatus.required).toBe(true));
    repairLookupFails = true;

    await act(async () => {
      await result.current.reload();
    });

    expect(result.current.repairStatus).toEqual({ required: true, state: 'pending', last_error: null });
    expect(result.current.error).toContain('repair journal unavailable');
  });

  it('keeps repair safety unknown after an initial repair-status lookup failure', async () => {
    callCommandMock.mockImplementation(async (name) => {
      if (name === 'get_dependency_status') {
        return [
          {
            package_name: 'dotnet48',
            state: 'missing',
            checked_at: null,
            installed_at: null,
            last_error: null,
          },
        ];
      }
      if (name === 'get_prefix_version_repair_status') throw new Error('repair journal unavailable');
      throw new Error(`unexpected command: ${name}`);
    });

    const { result } = renderHook(() => usePrefixDeps('Ghost', '/prefix'));

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.repairStatusKnown).toBe(false);
    expect(result.current.repairStatusError).toBe('repair journal unavailable');
  });
});
