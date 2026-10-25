import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { emitMockEvent, resetBrowserEventBus } from '@/lib/events';
import { resetStartupReportForTests } from '@/lib/startupReport';
import { makeStartupReport } from '@/test/startupReportFixture';
import type { EnrichedHealthSummary } from '@/types/health';
import type { ProfilesChangedPayload } from '@/types/profile-events';
import { useProfileHealth } from '../useProfileHealth';

const mockCallCommand = vi.fn();

vi.mock('@/lib/ipc', () => ({
  callCommand: (...args: unknown[]) => mockCallCommand(...args),
}));

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

function batchCalls() {
  return mockCallCommand.mock.calls.filter((c) => c[0] === 'batch_validate_profiles');
}

/** Renders with a complete startup report (zero batch calls) and drains every pending timer. */
async function renderSettled() {
  const report = makeStartupReport();
  const complete = { ...report, jobs: { ...report.jobs, health: { state: 'complete' as const, data: summary } } };
  mockCallCommand.mockImplementation(async (name: string) => {
    if (name === 'get_cached_health_snapshots') return [];
    if (name === 'get_startup_report') return complete;
    if (name === 'batch_validate_profiles') return summary;
    throw new Error(`unexpected ${name}`);
  });
  const hook = renderHook(() => useProfileHealth());
  await waitFor(() => expect(hook.result.current.summary).toEqual(summary));
  await drain();
  expect(batchCalls()).toHaveLength(0);
  return hook;
}

/** Negative assertions need a full drain or they pass vacuously. */
async function drain() {
  vi.useFakeTimers();
  await act(() => vi.runAllTimersAsync());
  vi.useRealTimers();
}

async function emitProfilesChanged(payload: unknown) {
  await act(async () => {
    emitMockEvent('profiles-changed', payload);
  });
  await drain();
}

describe('useProfileHealth profiles-changed policy', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStartupReportForTests();
    resetBrowserEventBus();
  });
  afterEach(() => vi.useRealTimers());

  it('skips batch validation for favorite_toggled', async () => {
    await renderSettled();
    const favorite: ProfilesChangedPayload = { reason: { kind: 'favorite_toggled' }, names: ['Alpha'] };
    await emitProfilesChanged(favorite);
    expect(batchCalls()).toHaveLength(0);
  });

  it('runs batch validation for saved (positive control)', async () => {
    await renderSettled();
    const saved: ProfilesChangedPayload = { reason: { kind: 'saved' }, names: ['Alpha'] };
    await emitProfilesChanged(saved);
    expect(batchCalls()).toHaveLength(1);
  });

  it.each([
    ['legacy empty string', ''],
    ['legacy name/action object', { name: 'Alpha', action: 'favorite' }],
    ['unknown kind', { reason: { kind: 'mystery' }, names: ['Alpha'] }],
    ['renamed without from', { reason: { kind: 'renamed' }, names: ['Alpha'] }],
    ['non-string names', { reason: { kind: 'favorite_toggled' }, names: [1] }],
    ['null payload', null],
  ])('treats a malformed payload (%s) as a conservative resync', async (_label, payload) => {
    await renderSettled();
    await emitProfilesChanged(payload);
    expect(batchCalls()).toHaveLength(1);
  });
});
