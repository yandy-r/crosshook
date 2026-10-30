import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { callCommand } from '@/lib/ipc';
import type { PrefixVersionRepairStatus } from '@/types/prefix-deps';
import { PREFIX_BUSY_POLL_MS, usePrefixDeps } from '../usePrefixDeps';

vi.mock('@/lib/ipc', () => ({
  callCommand: vi.fn(),
}));

const callCommandMock = vi.mocked(callCommand);

const CLEAN_REPAIR_STATUS: PrefixVersionRepairStatus = {
  required: false,
  state: null,
  last_error: null,
  in_progress: false,
  last_recovery: null,
};

describe('usePrefixDeps', () => {
  beforeEach(() => {
    callCommandMock.mockReset();
    callCommandMock.mockImplementation(async (name) => {
      if (name === 'get_dependency_status') return [];
      if (name === 'get_prefix_version_repair_status') {
        return CLEAN_REPAIR_STATUS;
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
          ? { ...CLEAN_REPAIR_STATUS, required: true, state: 'failed', last_error: 'restore timed out' }
          : CLEAN_REPAIR_STATUS;
      }
      if (name === 'repair_prefix_windows_version') {
        repairRequired = false;
        return CLEAN_REPAIR_STATUS;
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
        return { ...CLEAN_REPAIR_STATUS, required: true, state: 'failed', last_error: 'restore timed out' };
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
        return { ...CLEAN_REPAIR_STATUS, required: true, state: 'pending', last_error: null };
      }
      throw new Error(`unexpected command: ${name}`);
    });

    const { result } = renderHook(() => usePrefixDeps('Ghost', '/prefix'));
    await waitFor(() => expect(result.current.repairStatus.required).toBe(true));
    repairLookupFails = true;

    await act(async () => {
      await result.current.reload();
    });

    expect(result.current.repairStatus).toEqual({
      ...CLEAN_REPAIR_STATUS,
      required: true,
      state: 'pending',
      last_error: null,
    });
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

  it('re-polls the repair status while a job holds the prefix and reloads once it is released', async () => {
    vi.useFakeTimers();
    try {
      const busy: PrefixVersionRepairStatus = {
        ...CLEAN_REPAIR_STATUS,
        required: true,
        state: 'pending',
        in_progress: true,
      };
      const statuses = [busy, busy, CLEAN_REPAIR_STATUS];
      let lookups = 0;
      callCommandMock.mockImplementation(async (name) => {
        if (name === 'get_dependency_status') return [];
        if (name === 'get_prefix_version_repair_status') {
          const status = statuses[Math.min(lookups, statuses.length - 1)];
          lookups += 1;
          return status;
        }
        throw new Error(`unexpected command: ${name}`);
      });

      const { result } = renderHook(() => usePrefixDeps('Ghost', '/prefix'));
      await act(async () => {
        await vi.advanceTimersByTimeAsync(0);
      });
      expect(result.current.repairStatus.in_progress).toBe(true);
      expect(lookups).toBe(1);

      await act(async () => {
        await vi.advanceTimersByTimeAsync(PREFIX_BUSY_POLL_MS);
      });
      expect(lookups).toBe(2);
      expect(result.current.repairStatus.in_progress).toBe(true);

      await act(async () => {
        await vi.advanceTimersByTimeAsync(PREFIX_BUSY_POLL_MS);
      });
      // The released poll triggers a full reload of dependency + repair status.
      expect(lookups).toBe(4);
      expect(result.current.repairStatus).toEqual(CLEAN_REPAIR_STATUS);
      expect(callCommandMock.mock.calls.filter(([name]) => name === 'get_dependency_status')).toHaveLength(2);

      await act(async () => {
        await vi.advanceTimersByTimeAsync(PREFIX_BUSY_POLL_MS * 3);
      });
      expect(lookups).toBe(4);
    } finally {
      vi.useRealTimers();
    }
  });

  it('keeps polling when the reload after a released poll fails, then recovers', async () => {
    vi.useFakeTimers();
    try {
      const busy: PrefixVersionRepairStatus = {
        ...CLEAN_REPAIR_STATUS,
        required: true,
        state: 'pending',
        in_progress: true,
      };
      // 1: initial load (busy). 2: poll sees the prefix released. 3: the
      // reload's lookup fails. 4: next poll, released. 5: reload succeeds.
      const lookups: Array<PrefixVersionRepairStatus | Error> = [
        busy,
        CLEAN_REPAIR_STATUS,
        new Error('repair journal unavailable'),
        CLEAN_REPAIR_STATUS,
        CLEAN_REPAIR_STATUS,
      ];
      let calls = 0;
      callCommandMock.mockImplementation(async (name) => {
        if (name === 'get_dependency_status') return [];
        if (name === 'get_prefix_version_repair_status') {
          const next = lookups[Math.min(calls, lookups.length - 1)];
          calls += 1;
          if (next instanceof Error) throw next;
          return next;
        }
        throw new Error(`unexpected command: ${name}`);
      });

      const { result } = renderHook(() => usePrefixDeps('Ghost', '/prefix'));
      await act(async () => {
        await vi.advanceTimersByTimeAsync(0);
      });
      expect(calls).toBe(1);

      await act(async () => {
        await vi.advanceTimersByTimeAsync(PREFIX_BUSY_POLL_MS);
      });
      expect(calls).toBe(3);
      expect(result.current.repairStatusError).toBe('repair journal unavailable');
      expect(result.current.repairStatus.in_progress).toBe(true);

      await act(async () => {
        await vi.advanceTimersByTimeAsync(PREFIX_BUSY_POLL_MS);
      });
      expect(calls).toBe(5);
      expect(result.current.repairStatus).toEqual(CLEAN_REPAIR_STATUS);
      expect(result.current.repairStatusError).toBeNull();

      await act(async () => {
        await vi.advanceTimersByTimeAsync(PREFIX_BUSY_POLL_MS * 3);
      });
      expect(calls).toBe(5);
    } finally {
      vi.useRealTimers();
    }
  });

  it('keeps polling when the prefix is re-acquired between the poll and its reload', async () => {
    vi.useFakeTimers();
    try {
      const busy: PrefixVersionRepairStatus = {
        ...CLEAN_REPAIR_STATUS,
        required: true,
        state: 'pending',
        in_progress: true,
      };
      const lookups = [busy, CLEAN_REPAIR_STATUS, busy, CLEAN_REPAIR_STATUS, CLEAN_REPAIR_STATUS];
      let calls = 0;
      callCommandMock.mockImplementation(async (name) => {
        if (name === 'get_dependency_status') return [];
        if (name === 'get_prefix_version_repair_status') {
          const next = lookups[Math.min(calls, lookups.length - 1)];
          calls += 1;
          return next;
        }
        throw new Error(`unexpected command: ${name}`);
      });

      const { result } = renderHook(() => usePrefixDeps('Ghost', '/prefix'));
      await act(async () => {
        await vi.advanceTimersByTimeAsync(0);
      });
      expect(calls).toBe(1);

      await act(async () => {
        await vi.advanceTimersByTimeAsync(PREFIX_BUSY_POLL_MS);
      });
      expect(calls).toBe(3);
      expect(result.current.repairStatus.in_progress).toBe(true);

      await act(async () => {
        await vi.advanceTimersByTimeAsync(PREFIX_BUSY_POLL_MS);
      });
      expect(calls).toBe(5);
      expect(result.current.repairStatus).toEqual(CLEAN_REPAIR_STATUS);
    } finally {
      vi.useRealTimers();
    }
  });

  it('does not poll while a job holds the prefix without a repair row', async () => {
    vi.useFakeTimers();
    try {
      let calls = 0;
      callCommandMock.mockImplementation(async (name) => {
        if (name === 'get_dependency_status') return [];
        if (name === 'get_prefix_version_repair_status') {
          calls += 1;
          return { ...CLEAN_REPAIR_STATUS, in_progress: true };
        }
        throw new Error(`unexpected command: ${name}`);
      });

      renderHook(() => usePrefixDeps('Ghost', '/prefix'));
      // Let the initial load apply the status and arm any poll before advancing.
      await act(async () => {
        await vi.advanceTimersByTimeAsync(0);
      });
      await act(async () => {
        await vi.advanceTimersByTimeAsync(PREFIX_BUSY_POLL_MS * 3);
      });
      expect(calls).toBe(1);
    } finally {
      vi.useRealTimers();
    }
  });

  it('stops re-polling after unmount', async () => {
    vi.useFakeTimers();
    try {
      let lookups = 0;
      callCommandMock.mockImplementation(async (name) => {
        if (name === 'get_dependency_status') return [];
        if (name === 'get_prefix_version_repair_status') {
          lookups += 1;
          return { ...CLEAN_REPAIR_STATUS, required: true, state: 'pending', in_progress: true };
        }
        throw new Error(`unexpected command: ${name}`);
      });

      const { unmount } = renderHook(() => usePrefixDeps('Ghost', '/prefix'));
      await act(async () => {
        await vi.advanceTimersByTimeAsync(0);
      });
      expect(lookups).toBe(1);
      unmount();
      await vi.advanceTimersByTimeAsync(PREFIX_BUSY_POLL_MS * 3);
      expect(lookups).toBe(1);
    } finally {
      vi.useRealTimers();
    }
  });
});
