import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  CLOCK_SYNC_BUDGET_MS,
  CLOCK_SYNC_FAST_PAIRS,
  CLOCK_SYNC_MAX_SAMPLES,
  CLOCK_SYNC_SAMPLES,
  runClockSync,
} from '../clockSync';

const callCommandMock = vi.fn<(name: string, args?: Record<string, unknown>) => Promise<number>>();

vi.mock('@/lib/ipc', () => ({
  callCommand: (name: string, args?: Record<string, unknown>) => callCommandMock(name, args),
}));

/** A fake clock that advances `step(callIndex)` ms per bench_clock round trip. */
function steppedClock(step: (index: number) => number) {
  let t = 1000;
  let calls = 0;
  callCommandMock.mockImplementation(async () => {
    t += step(calls);
    calls += 1;
    return 0;
  });
  return () => t;
}

describe('runClockSync', () => {
  beforeEach(() => {
    callCommandMock.mockReset().mockResolvedValue(0);
  });

  it('sends sequential bench_clock samples with increasing seq and a perf stamp, one in flight', async () => {
    let inFlight = 0;
    let maxInFlight = 0;
    callCommandMock.mockImplementation(async () => {
      inFlight += 1;
      maxInFlight = Math.max(maxInFlight, inFlight);
      await Promise.resolve();
      inFlight -= 1;
      return 1;
    });

    await runClockSync(() => 5);

    expect(maxInFlight).toBe(1);
    callCommandMock.mock.calls.forEach(([name, args], index) => {
      expect(name).toBe('bench_clock');
      expect(args?.seq).toBe(index);
      expect(args?.tPerfMs).toBe(5);
    });
  });

  it('stops after the minimum when the app is quiet', async () => {
    const now = steppedClock(() => 0.5);
    await runClockSync(now);
    expect(callCommandMock).toHaveBeenCalledTimes(CLOCK_SYNC_FAST_PAIRS + 1);
  });

  it('keeps sampling while round trips are slow, then collects the fast pairs', async () => {
    const now = steppedClock((i) => (i < 30 ? 110 : 0.5));
    await runClockSync(now);
    expect(callCommandMock).toHaveBeenCalledTimes(30 + CLOCK_SYNC_FAST_PAIRS + 1);
  });

  it('gives up after the time budget once the minimum is sent', async () => {
    const now = steppedClock(() => 110);
    await runClockSync(now);
    // Call n ends at 110 * n ms after the start; the first n with 110 * n >= 30 000 is 273 (272 * 110 = 29 920).
    expect(callCommandMock).toHaveBeenCalledTimes(Math.ceil(CLOCK_SYNC_BUDGET_MS / 110));
    expect(callCommandMock).toHaveBeenCalledTimes(273);
    expect(callCommandMock.mock.calls.length).toBeGreaterThanOrEqual(CLOCK_SYNC_SAMPLES);
  });

  it('never exceeds the sample cap', async () => {
    const now = steppedClock(() => 3); // slower than the fast gap, and 400 x 3 ms stays inside the time budget
    await runClockSync(now);
    expect(callCommandMock).toHaveBeenCalledTimes(CLOCK_SYNC_MAX_SAMPLES);
  });

  it('propagates a failing sample', async () => {
    callCommandMock.mockRejectedValue(new Error('boom'));
    await expect(runClockSync()).rejects.toThrow('boom');
    expect(callCommandMock).toHaveBeenCalledTimes(1);
  });
});
