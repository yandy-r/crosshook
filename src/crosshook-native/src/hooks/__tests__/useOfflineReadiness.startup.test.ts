import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type * as Events from '@/lib/events';
import { emitMockEvent, resetBrowserEventBus } from '@/lib/events';
import { fetchStartupReport, resetStartupReportForTests } from '@/lib/startupReport';
import { makeStartupReport } from '@/test/startupReportFixture';
import type { OfflineReadinessReport } from '@/types/offline';
import type { StartupReport } from '@/types/startup';
import { useOfflineReadiness } from '../useOfflineReadiness';

const mockCallCommand = vi.fn();
const subscription = vi.hoisted(() => ({ gate: null as Promise<void> | null }));

vi.mock('@/lib/ipc', () => ({ callCommand: (...args: unknown[]) => mockCallCommand(...args) }));
vi.mock('@/lib/events', async (importOriginal) => {
  const actual = await importOriginal<typeof Events>();
  return {
    ...actual,
    subscribeEvent: async (...args: Parameters<typeof actual.subscribeEvent>) => {
      await subscription.gate;
      return actual.subscribeEvent(...args);
    },
  };
});

const reports: OfflineReadinessReport[] = [
  {
    profile_name: 'Alpha',
    score: 90,
    readiness_state: 'ready',
    trainer_type: 'standalone',
    checks: [],
    blocking_reasons: [],
    checked_at: '2020-01-01T00:00:00.000Z',
  },
];

function reportWith(job: StartupReport['jobs']['offline_readiness']): StartupReport {
  const report = makeStartupReport();
  return { ...report, jobs: { ...report.jobs, offline_readiness: job } };
}

function mockCommands(report: StartupReport | Promise<StartupReport>) {
  mockCallCommand.mockImplementation(async (name: string) => {
    if (name === 'get_cached_offline_readiness_snapshots') return [];
    if (name === 'get_startup_report') return report;
    if (name === 'batch_offline_readiness') return reports;
    throw new Error(`unexpected ${name}`);
  });
}

function batchCalls() {
  return mockCallCommand.mock.calls.filter((c) => c[0] === 'batch_offline_readiness');
}

async function settleReport() {
  await waitFor(() => expect(mockCallCommand).toHaveBeenCalledWith('get_startup_report'));
  vi.useFakeTimers();
  await act(() => vi.runAllTimersAsync());
  vi.useRealTimers();
}

describe('useOfflineReadiness startup report', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStartupReportForTests();
    resetBrowserEventBus();
    subscription.gate = null;
  });
  afterEach(() => vi.useRealTimers());

  it('runs batchCheck when the report job is complete', async () => {
    mockCommands(reportWith({ state: 'complete', data: { total_profiles: 1 } }));
    const { result } = renderHook(() => useOfflineReadiness());
    await waitFor(() => expect(result.current.reports).toEqual(reports));
    expect(batchCalls()).toHaveLength(1);
  });

  it('waits for the event after a pending report settles', async () => {
    mockCommands(reportWith({ state: 'pending' }));
    const { result } = renderHook(() => useOfflineReadiness());
    await settleReport();
    expect(batchCalls()).toHaveLength(0);
    await act(async () => {
      emitMockEvent('offline-readiness-scan-complete', { total_profiles: 1 });
    });
    await waitFor(() => expect(result.current.reports).toEqual(reports));
    expect(batchCalls()).toHaveLength(1);
  });

  it.each(['failed', 'skipped'] as const)('falls back once when the report is %s', async (state) => {
    mockCommands(reportWith(state === 'failed' ? { state, message: 'boom' } : { state, reason: 'disabled' }));
    const { result } = renderHook(() => useOfflineReadiness());
    await waitFor(() => expect(result.current.reports).toEqual(reports));
    expect(batchCalls()).toHaveLength(1);
  });

  it.each(['failed', 'skipped'] as const)('falls back once when a running job becomes %s', async (state) => {
    mockCommands(reportWith({ state: 'running' }));
    const { result } = renderHook(() => useOfflineReadiness());
    await settleReport();
    expect(batchCalls()).toHaveLength(0);
    await act(async () => {
      emitMockEvent('startup-job-finished', { job: 'offline_readiness', state });
      emitMockEvent('startup-job-finished', { job: 'offline_readiness', state });
    });
    await waitFor(() => expect(result.current.reports).toEqual(reports));
    expect(batchCalls()).toHaveLength(1);
  });

  it('deduplicates an event delivered before a complete report resolves', async () => {
    let resolveReport!: (report: StartupReport) => void;
    mockCommands(
      new Promise((resolve) => {
        resolveReport = resolve;
      })
    );
    const { result } = renderHook(() => useOfflineReadiness());
    await waitFor(() => expect(mockCallCommand).toHaveBeenCalledWith('get_startup_report'));
    await act(async () => {
      emitMockEvent('offline-readiness-scan-complete', { total_profiles: 1 });
      emitMockEvent('startup-job-finished', { job: 'offline_readiness', state: 'complete' });
      resolveReport(reportWith({ state: 'complete', data: { total_profiles: 1 } }));
    });
    await waitFor(() => expect(result.current.reports).toEqual(reports));
    expect(batchCalls()).toHaveLength(1);
  });

  it('handles success delivered before a late running report resolves', async () => {
    let resolveReport!: (report: StartupReport) => void;
    mockCommands(
      new Promise((resolve) => {
        resolveReport = resolve;
      })
    );
    const { result } = renderHook(() => useOfflineReadiness());
    await waitFor(() => expect(mockCallCommand).toHaveBeenCalledWith('get_startup_report'));
    await act(async () => {
      emitMockEvent('offline-readiness-scan-complete', { total_profiles: 1 });
      resolveReport(reportWith({ state: 'running' }));
    });
    await waitFor(() => expect(result.current.reports).toEqual(reports));
    expect(batchCalls()).toHaveLength(1);
  });

  it('reads a fresh complete report on late mount despite a memoized pending report', async () => {
    mockCommands(reportWith({ state: 'pending' }));
    await fetchStartupReport();
    const first = renderHook(() => useOfflineReadiness());
    await settleReport();
    await act(async () => {
      emitMockEvent('offline-readiness-scan-complete', { total_profiles: 1 });
    });
    first.unmount();
    mockCommands(reportWith({ state: 'complete', data: { total_profiles: 1 } }));
    const second = renderHook(() => useOfflineReadiness());
    await waitFor(() => expect(second.result.current.reports).toEqual(reports));
    expect(mockCallCommand.mock.calls.filter((c) => c[0] === 'get_startup_report')).toHaveLength(3);
    expect(batchCalls()).toHaveLength(2);
  });

  it('does not read the report until listener registration completes', async () => {
    let register!: () => void;
    subscription.gate = new Promise((resolve) => {
      register = resolve;
    });
    mockCommands(reportWith({ state: 'complete', data: { total_profiles: 1 } }));
    const { result } = renderHook(() => useOfflineReadiness());
    await waitFor(() => expect(mockCallCommand).toHaveBeenCalledWith('get_cached_offline_readiness_snapshots'));
    vi.useFakeTimers();
    await act(() => vi.runAllTimersAsync());
    expect(mockCallCommand).not.toHaveBeenCalledWith('get_startup_report');
    await act(async () => {
      register();
    });
    vi.useRealTimers();
    await waitFor(() => expect(result.current.reports).toEqual(reports));
  });
});
