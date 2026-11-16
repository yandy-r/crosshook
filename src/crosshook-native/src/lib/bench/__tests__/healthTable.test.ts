import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { resetBenchWarning } from '../benchMark';
import {
  HEALTH_MIN_ROWS,
  HEALTH_WAIT_MS,
  liveHealthRowCount,
  openRoute,
  reportHealthTable,
  waitForHealthRows,
} from '../healthTable';
import { appendLiveTable, buildRoutes, CACHED_LABEL, healthTableHtml } from './healthFixture';

const callCommandMock = vi.fn<(name: string, args?: Record<string, unknown>) => Promise<unknown>>();

vi.mock('@/lib/ipc', () => ({
  callCommand: (name: string, args?: Record<string, unknown>) => callCommandMock(name, args),
}));

const marks = () =>
  callCommandMock.mock.calls
    .filter(([name]) => name === 'bench_mark')
    .map(([, args]) => args as Record<string, unknown>);

beforeEach(() => {
  resetBenchWarning();
  callCommandMock.mockReset().mockResolvedValue(undefined);
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  document.body.innerHTML = '';
});

describe('liveHealthRowCount', () => {
  it('is 0 without a table', () => {
    expect(liveHealthRowCount()).toBe(0);
  });

  it('counts profile rows only, not detail rows, and ignores the cached table', () => {
    document.body.innerHTML = healthTableHtml(CACHED_LABEL, 7);
    expect(liveHealthRowCount()).toBe(0);
    appendLiveTable(document.body, 3, 2);
    expect(liveHealthRowCount()).toBe(3);
  });
});

describe('waitForHealthRows', () => {
  it('resolves at once when the table is already full', async () => {
    appendLiveTable(document.body, HEALTH_MIN_ROWS);
    await expect(waitForHealthRows(1000)).resolves.toBe(HEALTH_MIN_ROWS);
  });

  it('resolves when the live table reaches the minimum', async () => {
    const pending = waitForHealthRows(5000);
    appendLiveTable(document.body, HEALTH_MIN_ROWS + 1);
    await expect(pending).resolves.toBe(HEALTH_MIN_ROWS + 1);
  });

  it('keeps waiting at 299 rows and returns the last count at the timeout', async () => {
    vi.useFakeTimers();
    const pending = waitForHealthRows(1000);
    appendLiveTable(document.body, HEALTH_MIN_ROWS - 1);
    await vi.advanceTimersByTimeAsync(999);
    let settled = false;
    void pending.then(() => {
      settled = true;
    });
    await vi.advanceTimersByTimeAsync(0);
    expect(settled).toBe(false);
    await vi.advanceTimersByTimeAsync(1);
    await expect(pending).resolves.toBe(HEALTH_MIN_ROWS - 1);
  });

  it('does not count a cached-only table', async () => {
    vi.useFakeTimers();
    const pending = waitForHealthRows(1000);
    document.body.insertAdjacentHTML('beforeend', healthTableHtml(CACHED_LABEL, 500));
    await vi.advanceTimersByTimeAsync(1000);
    await expect(pending).resolves.toBe(0);
  });

  it('disconnects its observer and timer on abort, on timeout and on success', async () => {
    const disconnect = vi.spyOn(MutationObserver.prototype, 'disconnect');
    vi.useFakeTimers();

    const controller = new AbortController();
    const aborted = waitForHealthRows(HEALTH_WAIT_MS, controller.signal);
    controller.abort();
    await expect(aborted).resolves.toBe(0);
    expect(disconnect).toHaveBeenCalledTimes(1);
    expect(vi.getTimerCount()).toBe(0);

    const timedOut = waitForHealthRows(10);
    await vi.advanceTimersByTimeAsync(10);
    await timedOut;
    expect(disconnect).toHaveBeenCalledTimes(2);

    const done = waitForHealthRows(HEALTH_WAIT_MS);
    appendLiveTable(document.body, HEALTH_MIN_ROWS);
    await vi.advanceTimersByTimeAsync(0);
    await done;
    expect(disconnect).toHaveBeenCalledTimes(3);
    expect(vi.getTimerCount()).toBe(0);
  });

  it('resolves 0 for an already-aborted signal without observing', async () => {
    const controller = new AbortController();
    controller.abort();
    const observe = vi.spyOn(MutationObserver.prototype, 'observe');
    await expect(waitForHealthRows(1000, controller.signal)).resolves.toBe(0);
    expect(observe).not.toHaveBeenCalled();
  });
});

describe('openRoute', () => {
  it('activates the trigger and resolves once the panel is mounted', async () => {
    buildRoutes({ healthRows: 1 });
    await expect(openRoute('health')).resolves.toBe(true);
    expect(document.getElementById('r-trigger-health')?.getAttribute('aria-selected')).toBe('true');
  });

  it('resolves false without a trigger', async () => {
    document.body.innerHTML = '';
    await expect(openRoute('health')).resolves.toBe(false);
  });
});

describe('reportHealthTable', () => {
  it('opens health and sends health_ready at seq 0 once the live table is full', async () => {
    buildRoutes({ healthRows: HEALTH_MIN_ROWS });
    await expect(reportHealthTable('health_table')).resolves.toBe(true);
    expect(marks()).toEqual([{ scenario: 'health_table', seq: 0, phase: 'health_ready', tPerfMs: expect.any(Number) }]);
  });

  it('sends health_rows_too_few and returns false at the timeout', async () => {
    vi.useFakeTimers();
    buildRoutes({ healthRows: HEALTH_MIN_ROWS - 1 });
    const pending = reportHealthTable('health_scroll', undefined, 500);
    await vi.advanceTimersByTimeAsync(600);
    await expect(pending).resolves.toBe(false);
    expect(marks().map((m) => [m.scenario, m.phase, m.seq])).toEqual([['health_scroll', 'health_rows_too_few', 0]]);
  });

  it('sends no mark when the health trigger is missing', async () => {
    await expect(reportHealthTable('health_table')).resolves.toBe(false);
    expect(marks()).toEqual([]);
  });

  it('sends no mark when aborted while waiting', async () => {
    buildRoutes({ healthRows: 5 });
    const controller = new AbortController();
    const pending = reportHealthTable('health_table', controller.signal);
    await new Promise((resolve) => setTimeout(resolve, 20));
    controller.abort();
    await expect(pending).resolves.toBe(false);
    expect(marks()).toEqual([]);
  });

  it('returns false when the mark cannot be sent', async () => {
    vi.spyOn(console, 'warn').mockImplementation(() => {});
    callCommandMock.mockRejectedValue(new Error('rejected'));
    buildRoutes({ healthRows: HEALTH_MIN_ROWS });
    await expect(reportHealthTable('health_table')).resolves.toBe(false);
  });
});
