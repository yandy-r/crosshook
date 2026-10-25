import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { resetStartupReportForTests } from '@/lib/startupReport';
import { makeStartupReport } from '@/test/startupReportFixture';
import { FLATPAK_MIGRATION_TOAST_SESSION_KEY, useFlatpakMigrationToast } from '../useFlatpakMigrationToast';

const mockCallCommand = vi.fn();

vi.mock('@/lib/ipc', () => ({
  callCommand: (...args: unknown[]) => mockCallCommand(...args),
}));

const fullMigration = {
  imported_config: true,
  imported_subtrees: ['profiles', 'scripts'],
  skipped_subtrees: [],
  error: null,
};

const emptyMigration = {
  imported_config: false,
  imported_subtrees: [],
  skipped_subtrees: [],
  error: null,
};

function mockReport(migration: typeof fullMigration | null) {
  mockCallCommand.mockResolvedValue(
    makeStartupReport({ flatpak: { mode: migration === null ? 'native' : 'isolated', migration } })
  );
}

describe('useFlatpakMigrationToast', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStartupReportForTests();
    try {
      sessionStorage.removeItem(FLATPAK_MIGRATION_TOAST_SESSION_KEY);
    } catch {
      // ignore
    }
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('shows toast from report migration (2 subtrees + config = 3)', async () => {
    mockReport(fullMigration);
    const { result } = renderHook(() => useFlatpakMigrationToast());
    await waitFor(() => expect(result.current.importCount).toBe(3));
  });

  // Flush until the mocked report promise has actually settled, so a
  // regression that shows the toast on an empty/null migration would fail.
  async function settleReport() {
    await waitFor(() => expect(mockCallCommand).toHaveBeenCalledWith('get_startup_report'));
    vi.useFakeTimers();
    try {
      await act(() => vi.runAllTimersAsync());
    } finally {
      vi.useRealTimers();
    }
  }

  it('no-op when migration is null', async () => {
    mockReport(null);
    const { result } = renderHook(() => useFlatpakMigrationToast());
    await settleReport();
    expect(result.current.importCount).toBeNull();
  });

  it('no-op when outcome is empty', async () => {
    mockReport(emptyMigration);
    const { result } = renderHook(() => useFlatpakMigrationToast());
    await settleReport();
    expect(result.current.importCount).toBeNull();
  });

  it('counts only subtrees when imported_config is false', async () => {
    mockReport({ imported_config: false, imported_subtrees: ['a', 'b', 'c'], skipped_subtrees: [], error: null });
    const { result } = renderHook(() => useFlatpakMigrationToast());
    await waitFor(() => expect(result.current.importCount).toBe(3));
  });

  it('dedupes via sessionStorage — reload (claim reset) does not re-show', async () => {
    mockReport(fullMigration);
    const first = renderHook(() => useFlatpakMigrationToast());
    await waitFor(() => expect(first.result.current.importCount).toBe(3));
    act(() => {
      first.result.current.dismiss();
    });
    expect(first.result.current.importCount).toBeNull();
    first.unmount();

    // Simulate a page reload: claims and the memoized report are page-load
    // state, so only sessionStorage can dedupe the second mount.
    resetStartupReportForTests();
    mockReport(fullMigration);

    const second = renderHook(() => useFlatpakMigrationToast());
    await settleReport();
    expect(second.result.current.importCount).toBeNull();
  });

  it('claim prevents duplicate toast within one page load even without sessionStorage', async () => {
    mockReport(fullMigration);
    const first = renderHook(() => useFlatpakMigrationToast());
    await waitFor(() => expect(first.result.current.importCount).toBe(3));
    try {
      sessionStorage.removeItem(FLATPAK_MIGRATION_TOAST_SESSION_KEY);
    } catch {
      // ignore
    }
    first.unmount();
    const second = renderHook(() => useFlatpakMigrationToast());
    await settleReport();
    expect(second.result.current.importCount).toBeNull();
  });

  it('dismiss sets importCount to null and writes sessionStorage', async () => {
    mockReport(fullMigration);
    const { result } = renderHook(() => useFlatpakMigrationToast());
    await waitFor(() => expect(result.current.importCount).toBe(3));
    act(() => {
      result.current.dismiss();
    });
    expect(result.current.importCount).toBeNull();
    expect(sessionStorage.getItem(FLATPAK_MIGRATION_TOAST_SESSION_KEY)).toBe('1');
  });
});
