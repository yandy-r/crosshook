import { beforeEach, describe, expect, it, vi } from 'vitest';

const mockCallCommand = vi.fn();

vi.mock('@/lib/ipc', () => ({
  callCommand: (...args: unknown[]) => mockCallCommand(...args),
}));

import { makeStartupReport } from '@/test/startupReportFixture';
import { claimStartupAction, fetchStartupReport, resetStartupReportForTests } from '../startupReport';

function reportWith(autoLoad: string | null) {
  return makeStartupReport({ auto_load_profile: autoLoad });
}

describe('startupReport', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStartupReportForTests();
  });

  it('memoizes fetchStartupReport across calls', async () => {
    mockCallCommand.mockResolvedValue(reportWith('Alpha'));
    const first = await fetchStartupReport();
    const second = await fetchStartupReport();
    expect(first).toBe(second);
    expect(mockCallCommand).toHaveBeenCalledTimes(1);
    expect(mockCallCommand).toHaveBeenCalledWith('get_startup_report');
  });

  it('claims each startup action only once per page load', () => {
    expect(claimStartupAction('auto_load_profile')).toBe(true);
    expect(claimStartupAction('auto_load_profile')).toBe(false);
    expect(claimStartupAction('onboarding_prompt')).toBe(true);
    expect(claimStartupAction('onboarding_prompt')).toBe(false);
    expect(claimStartupAction('flatpak_migration_notice')).toBe(true);
    expect(claimStartupAction('flatpak_migration_notice')).toBe(false);
  });

  it('clears the memo when the fetch rejects so a later call retries', async () => {
    mockCallCommand.mockRejectedValueOnce(new Error('boom'));
    await expect(fetchStartupReport()).rejects.toThrow('boom');
    mockCallCommand.mockResolvedValue(reportWith(null));
    await expect(fetchStartupReport()).resolves.toMatchObject({ auto_load_profile: null });
    expect(mockCallCommand).toHaveBeenCalledTimes(2);
  });

  it('fresh fetch bypasses the memo without replacing the cached promise', async () => {
    mockCallCommand.mockResolvedValueOnce(reportWith('Cached'));
    const cached = await fetchStartupReport();
    expect(cached.auto_load_profile).toBe('Cached');

    mockCallCommand.mockResolvedValueOnce(reportWith('Fresh'));
    const fresh = await fetchStartupReport({ fresh: true });
    expect(fresh.auto_load_profile).toBe('Fresh');
    expect(mockCallCommand).toHaveBeenCalledTimes(2);

    // The memo still holds the first result: a non-fresh call returns it
    // without another IPC call.
    const again = await fetchStartupReport();
    expect(again).toBe(cached);
    expect(mockCallCommand).toHaveBeenCalledTimes(2);
  });

  it('reset clears memo and claims', async () => {
    mockCallCommand.mockResolvedValue(reportWith('Alpha'));
    await fetchStartupReport();
    expect(claimStartupAction('auto_load_profile')).toBe(true);
    resetStartupReportForTests();
    expect(claimStartupAction('auto_load_profile')).toBe(true);
    await fetchStartupReport();
    expect(mockCallCommand).toHaveBeenCalledTimes(2);
  });
});
