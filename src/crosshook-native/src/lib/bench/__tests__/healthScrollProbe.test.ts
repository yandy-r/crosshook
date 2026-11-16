import { afterEach, beforeEach, describe, expect, it, type MockInstance, vi } from 'vitest';
import { resetBenchWarning } from '../benchMark';
import { HEALTH_SCROLL_SCENARIO, startHealthScrollProbe } from '../healthScrollProbe';
import { HEALTH_MIN_ROWS } from '../healthTable';
import { buildRoutes } from './healthFixture';

const callCommandMock = vi.fn<(name: string, args?: Record<string, unknown>) => Promise<unknown>>();

vi.mock('@/lib/ipc', () => ({
  callCommand: (name: string, args?: Record<string, unknown>) => callCommandMock(name, args),
}));

const names = () => callCommandMock.mock.calls.map(([name]) => name);
const phases = () =>
  callCommandMock.mock.calls
    .filter(([name]) => name === 'bench_mark')
    .map(([, args]) => `${(args as Record<string, unknown>).scenario}:${(args as Record<string, unknown>).phase}`);

let scrollIntoView: MockInstance<typeof HTMLElement.prototype.scrollIntoView>;

beforeEach(() => {
  resetBenchWarning();
  callCommandMock.mockReset().mockResolvedValue(undefined);
  scrollIntoView = vi.spyOn(window.HTMLElement.prototype, 'scrollIntoView').mockImplementation(() => {});
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  document.body.innerHTML = '';
});

describe('startHealthScrollProbe', () => {
  it('sends health_ready, scrolls the live table to the top, then calls bench_begin without clock sync', async () => {
    buildRoutes({ healthRows: HEALTH_MIN_ROWS });
    await startHealthScrollProbe();
    expect(names()).toEqual(['bench_mark', 'bench_begin']);
    expect(phases()).toEqual([`${HEALTH_SCROLL_SCENARIO}:health_ready`]);
    expect(scrollIntoView).toHaveBeenCalledWith({ block: 'start', behavior: 'auto' });
    expect(scrollIntoView.mock.contexts[0]).toBe(document.querySelector('table[aria-label="Profile health status"]'));
  });

  it('sends health_rows_too_few and never calls bench_begin with too few rows', async () => {
    vi.useFakeTimers();
    buildRoutes({ healthRows: HEALTH_MIN_ROWS - 1 });
    const probe = startHealthScrollProbe({ timeoutMs: 500 });
    await vi.advanceTimersByTimeAsync(600);
    await expect(probe).resolves.toBeUndefined();
    expect(phases()).toEqual([`${HEALTH_SCROLL_SCENARIO}:health_rows_too_few`]);
    expect(names()).not.toContain('bench_begin');
    expect(scrollIntoView).not.toHaveBeenCalled();
  });

  it('does nothing and does not reject when aborted', async () => {
    buildRoutes({ healthRows: HEALTH_MIN_ROWS });
    const controller = new AbortController();
    controller.abort();
    await expect(startHealthScrollProbe({ signal: controller.signal })).resolves.toBeUndefined();
    expect(callCommandMock).not.toHaveBeenCalled();
  });

  it('warns once and resolves when bench_begin rejects', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    callCommandMock.mockImplementation(async (name) => {
      if (name === 'bench_begin') {
        throw new Error('nope');
      }
    });
    buildRoutes({ healthRows: HEALTH_MIN_ROWS });
    await expect(startHealthScrollProbe()).resolves.toBeUndefined();
    expect(warn).toHaveBeenCalledTimes(1);
  });
});
