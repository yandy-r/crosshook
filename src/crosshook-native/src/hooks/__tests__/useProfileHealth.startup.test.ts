import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type * as Events from '@/lib/events';
import { emitMockEvent, resetBrowserEventBus } from '@/lib/events';
import { fetchStartupReport, resetStartupReportForTests } from '@/lib/startupReport';
import { makeStartupReport } from '@/test/startupReportFixture';
import type { EnrichedHealthSummary } from '@/types/health';
import type { StartupReport } from '@/types/startup';
import { useProfileHealth } from '../useProfileHealth';

const mockCallCommand = vi.fn();
const subscription = vi.hoisted(() => ({ gate: null as Promise<void> | null }));

vi.mock('@/lib/ipc', () => ({
  callCommand: (...args: unknown[]) => mockCallCommand(...args),
}));
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

const summary: EnrichedHealthSummary = {
  profiles: [
    {
      name: 'Alpha',
      status: 'healthy',
      launch_method: 'steam',
      issues: [],
      checked_at: '2020-01-01T00:00:00.000Z',
      metadata: null,
    },
  ],
  healthy_count: 1,
  stale_count: 0,
  broken_count: 0,
  total_count: 1,
  validated_at: '2020-01-01T00:00:00.000Z',
};

function reportWith(job: StartupReport['jobs']['health']): StartupReport {
  const report = makeStartupReport();
  return { ...report, jobs: { ...report.jobs, health: job } };
}

function mockCommands(report: StartupReport | Promise<StartupReport>) {
  mockCallCommand.mockImplementation(async (name: string) => {
    if (name === 'get_cached_health_snapshots') return [];
    if (name === 'get_startup_report') return report;
    if (name === 'batch_validate_profiles') return summary;
    throw new Error(`unexpected ${name}`);
  });
}

function batchCalls() {
  return mockCallCommand.mock.calls.filter((c) => c[0] === 'batch_validate_profiles');
}

/** Sequential reads: the mount flow reads the first, re-reads consume the rest in order. */
function sequentialReports(...reports: StartupReport[]) {
  let read = 0;
  mockCallCommand.mockImplementation(async (name: string) => {
    if (name === 'get_cached_health_snapshots') return [];
    if (name === 'get_startup_report') {
      const report = reports[read] ?? reports[reports.length - 1];
      read += 1;
      return report;
    }
    if (name === 'batch_validate_profiles') return summary;
    throw new Error(`unexpected ${name}`);
  });
}

async function settleReport() {
  await waitFor(() => expect(mockCallCommand).toHaveBeenCalledWith('get_startup_report'));
  vi.useFakeTimers();
  await act(() => vi.runAllTimersAsync());
  vi.useRealTimers();
}

describe('useProfileHealth startup report', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStartupReportForTests();
    resetBrowserEventBus();
    subscription.gate = null;
  });
  afterEach(() => vi.useRealTimers());

  it('applies complete health from the report before any event', async () => {
    mockCommands(reportWith({ state: 'complete', data: summary }));
    const { result } = renderHook(() => useProfileHealth());
    await waitFor(() => expect(result.current.summary).toEqual(summary));
    expect(batchCalls()).toHaveLength(0);
  });

  it('waits for the event after a pending report settles', async () => {
    mockCommands(reportWith({ state: 'pending' }));
    const { result } = renderHook(() => useProfileHealth());
    await settleReport();
    expect(result.current.summary).toBeNull();
    expect(batchCalls()).toHaveLength(0);
    await act(async () => {
      emitMockEvent('profile-health-batch-complete', summary);
    });
    expect(result.current.summary).toEqual(summary);
  });

  it.each(['failed', 'skipped'] as const)('falls back once when the report is %s', async (state) => {
    mockCommands(reportWith(state === 'failed' ? { state, message: 'boom' } : { state, reason: 'disabled' }));
    const { result } = renderHook(() => useProfileHealth());
    await waitFor(() => expect(result.current.summary).toEqual(summary));
    expect(batchCalls()).toHaveLength(1);
  });

  it.each(['failed', 'skipped'] as const)('falls back once when a running job becomes %s', async (state) => {
    mockCommands(reportWith({ state: 'running' }));
    const { result } = renderHook(() => useProfileHealth());
    await settleReport();
    expect(batchCalls()).toHaveLength(0);
    await act(async () => {
      emitMockEvent('startup-job-finished', { job: 'health_batch', state });
      emitMockEvent('startup-job-finished', { job: 'health_batch', state });
    });
    await waitFor(() => expect(result.current.summary).toEqual(summary));
    expect(batchCalls()).toHaveLength(1);
  });

  it('re-reads the report when a running job signals complete without the legacy event', async () => {
    sequentialReports(reportWith({ state: 'running' }), reportWith({ state: 'complete', data: summary }));
    const { result } = renderHook(() => useProfileHealth());
    await settleReport();
    expect(result.current.summary).toBeNull();
    expect(batchCalls()).toHaveLength(0);
    await act(async () => {
      emitMockEvent('startup-job-finished', { job: 'health_batch', state: 'complete' });
    });
    await waitFor(() => expect(result.current.summary).toEqual(summary));
    expect(batchCalls()).toHaveLength(0);
    expect(mockCallCommand.mock.calls.filter((c) => c[0] === 'get_startup_report')).toHaveLength(2);
  });

  it('falls back once when the re-read triggered by a complete signal rejects', async () => {
    let failReads = false;
    mockCallCommand.mockImplementation(async (name: string) => {
      if (name === 'get_cached_health_snapshots') return [];
      if (name === 'get_startup_report') {
        if (failReads) throw new Error('read failed');
        return reportWith({ state: 'running' });
      }
      if (name === 'batch_validate_profiles') return summary;
      throw new Error(`unexpected ${name}`);
    });
    const { result } = renderHook(() => useProfileHealth());
    await settleReport();
    failReads = true;
    await act(async () => {
      emitMockEvent('startup-job-finished', { job: 'health_batch', state: 'complete' });
      emitMockEvent('startup-job-finished', { job: 'health_batch', state: 'complete' });
    });
    await waitFor(() => expect(result.current.summary).toEqual(summary));
    expect(batchCalls()).toHaveLength(1);
  });

  it('dispatches once when duplicate complete signals race their report reads', async () => {
    const first = reportWith({ state: 'complete', data: summary });
    const later = reportWith({
      state: 'complete',
      data: { ...summary, validated_at: '2021-01-01T00:00:00.000Z' },
    });
    const pending: Array<(report: StartupReport) => void> = [];
    mockCallCommand.mockImplementation((name: string) => {
      if (name === 'get_cached_health_snapshots') return Promise.resolve([]);
      if (name === 'get_startup_report') {
        return new Promise<StartupReport>((resolve) => {
          pending.push(resolve);
        });
      }
      return Promise.reject(new Error(`unexpected ${name}`));
    });
    const { result } = renderHook(() => useProfileHealth());
    await settleReport();
    await act(async () => {
      emitMockEvent('startup-job-finished', { job: 'health_batch', state: 'complete' });
      emitMockEvent('startup-job-finished', { job: 'health_batch', state: 'complete' });
    });
    expect(result.current.summary).toBeNull();
    await act(async () => {
      // First read belongs to the mount flow; racing duplicate signals must
      // not overwrite its result.
      pending[0]?.(first);
      for (const resolve of pending.slice(1)) resolve(later);
    });
    expect(result.current.summary).toEqual(summary);
  });

  it('ignores a stale legacy replay after startup data was already applied', async () => {
    const replayed: EnrichedHealthSummary = {
      ...summary,
      validated_at: '1999-01-01T00:00:00.000Z',
    };
    sequentialReports(reportWith({ state: 'running' }), reportWith({ state: 'complete', data: summary }));
    const { result } = renderHook(() => useProfileHealth());
    await settleReport();
    await act(async () => {
      emitMockEvent('startup-job-finished', { job: 'health_batch', state: 'complete' });
    });
    await waitFor(() => expect(result.current.summary).toEqual(summary));
    await act(async () => {
      emitMockEvent('profile-health-batch-complete', replayed);
    });
    expect(result.current.summary).toEqual(summary);
  });

  it('ignores a delayed legacy event after startup data was already applied', async () => {
    mockCommands(reportWith({ state: 'complete', data: summary }));
    const { result } = renderHook(() => useProfileHealth());
    await waitFor(() => expect(result.current.summary).toEqual(summary));
    await act(async () => {
      emitMockEvent('profile-health-batch-complete', summary);
    });
    expect(result.current.summary).toEqual(summary);
    expect(batchCalls()).toHaveLength(0);
  });

  it('retains success delivered before a late running report resolves', async () => {
    let resolveReport!: (report: StartupReport) => void;
    mockCommands(
      new Promise((resolve) => {
        resolveReport = resolve;
      })
    );
    const { result } = renderHook(() => useProfileHealth());
    await waitFor(() => expect(mockCallCommand).toHaveBeenCalledWith('get_startup_report'));
    await act(async () => {
      emitMockEvent('profile-health-batch-complete', summary);
      resolveReport(reportWith({ state: 'running' }));
    });
    expect(result.current.summary).toEqual(summary);
    expect(batchCalls()).toHaveLength(0);
  });

  it('reads a fresh complete report for a late mount despite a memoized running report', async () => {
    mockCommands(reportWith({ state: 'running' }));
    await fetchStartupReport();
    const first = renderHook(() => useProfileHealth());
    await settleReport();
    await act(async () => {
      emitMockEvent('profile-health-batch-complete', summary);
    });
    first.unmount();
    mockCommands(reportWith({ state: 'complete', data: summary }));
    const second = renderHook(() => useProfileHealth());
    await waitFor(() => expect(second.result.current.summary).toEqual(summary));
    expect(mockCallCommand.mock.calls.filter((c) => c[0] === 'get_startup_report')).toHaveLength(3);
    expect(batchCalls()).toHaveLength(0);
  });

  it('does not read the report until listener registration completes', async () => {
    let register!: () => void;
    subscription.gate = new Promise((resolve) => {
      register = resolve;
    });
    mockCommands(reportWith({ state: 'complete', data: summary }));
    const { result } = renderHook(() => useProfileHealth());
    await waitFor(() => expect(mockCallCommand).toHaveBeenCalledWith('get_cached_health_snapshots'));
    vi.useFakeTimers();
    await act(() => vi.runAllTimersAsync());
    expect(mockCallCommand).not.toHaveBeenCalledWith('get_startup_report');
    await act(async () => {
      register();
    });
    vi.useRealTimers();
    await waitFor(() => expect(result.current.summary).toEqual(summary));
  });

  describe('a background batch that finishes first', () => {
    const newer: EnrichedHealthSummary = { ...summary, validated_at: '2030-01-01T00:00:00.000Z' };
    const saved = { reason: { kind: 'saved' }, names: ['Alpha'] };

    /** Startup reads resolve through `reports` in call order (deferred); batches return `newer`. */
    function deferredReports() {
      const reports: Array<(report: StartupReport) => void> = [];
      mockCallCommand.mockImplementation((name: string) => {
        if (name === 'get_cached_health_snapshots') return Promise.resolve([]);
        if (name === 'get_startup_report') {
          const gate = Promise.withResolvers<StartupReport>();
          reports.push(gate.resolve);
          return gate.promise;
        }
        if (name === 'batch_validate_profiles') return Promise.resolve(newer);
        return Promise.reject(new Error(`unexpected ${name}`));
      });
      return reports;
    }

    it('is not overwritten by the older mount report or a late legacy event', async () => {
      const reports = deferredReports();
      const { result } = renderHook(() => useProfileHealth());
      await waitFor(() => expect(reports).toHaveLength(1));
      await act(async () => {
        emitMockEvent('profiles-changed', saved);
      });
      await waitFor(() => expect(result.current.summary).toEqual(newer));

      await act(async () => {
        reports[0]?.(reportWith({ state: 'complete', data: summary }));
      });
      await act(async () => {
        emitMockEvent('profile-health-batch-complete', summary);
        emitMockEvent('startup-job-finished', { job: 'health_batch', state: 'complete' });
      });
      await act(() => Promise.resolve());

      expect(result.current.summary).toEqual(newer);
      expect(reports).toHaveLength(1);
      expect(batchCalls()).toHaveLength(1);
    });

    it('is not overwritten by a completion re-read that resolves later', async () => {
      const reports = deferredReports();
      const { result } = renderHook(() => useProfileHealth());
      await waitFor(() => expect(reports).toHaveLength(1));
      await act(async () => {
        reports[0]?.(reportWith({ state: 'running' }));
      });
      await act(async () => {
        emitMockEvent('startup-job-finished', { job: 'health_batch', state: 'complete' });
      });
      await waitFor(() => expect(reports).toHaveLength(2));
      await act(async () => {
        emitMockEvent('profiles-changed', saved);
      });
      await waitFor(() => expect(result.current.summary).toEqual(newer));

      await act(async () => {
        reports[1]?.(reportWith({ state: 'complete', data: summary }));
      });

      expect(result.current.summary).toEqual(newer);
      expect(batchCalls()).toHaveLength(1);
    });
  });
});
