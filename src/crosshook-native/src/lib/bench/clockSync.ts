import { callCommand } from '@/lib/ipc';

/** Minimum number of `bench_clock` samples the runner correlates against its own monotonic clock. */
export const CLOCK_SYNC_SAMPLES = 20;
/** A gap between consecutive samples at or below this is a fast round trip. */
const CLOCK_SYNC_FAST_GAP_MS = 2;
/** Fast round trips wanted before sampling stops. */
export const CLOCK_SYNC_FAST_PAIRS = 20;
/** Hard caps: sampling stops after this many samples or this long, fast or not. */
export const CLOCK_SYNC_MAX_SAMPLES = 400;
export const CLOCK_SYNC_BUDGET_MS = 30_000;

/**
 * Sends sequential `bench_clock` calls, each carrying the webview `performance.now()`
 * read immediately before the call. The runner reads the matching stderr line, so
 * the return value of each call is ignored.
 *
 * While the app is still busy (a large library rendering after READY) every round
 * trip is slow, and a slow pair cannot pin the clock offset. So sampling continues
 * past `CLOCK_SYNC_SAMPLES` until `CLOCK_SYNC_FAST_PAIRS` consecutive-sample gaps were
 * at most `CLOCK_SYNC_FAST_GAP_MS`, or a cap is reached; the runner keeps only the
 * fast pairs. Bench-only: called solely from the benchmark probe.
 */
export async function runClockSync(now: () => number = () => performance.now()): Promise<void> {
  const start = now();
  let previous: number | null = null;
  let fastPairs = 0;
  for (let seq = 0; seq < CLOCK_SYNC_MAX_SAMPLES; seq += 1) {
    const stamp = now();
    if (previous !== null && stamp - previous <= CLOCK_SYNC_FAST_GAP_MS) {
      fastPairs += 1;
    }
    previous = stamp;
    await callCommand<number>('bench_clock', { seq, tPerfMs: stamp });
    const sent = seq + 1;
    if (fastPairs >= CLOCK_SYNC_FAST_PAIRS) {
      return;
    }
    if (sent >= CLOCK_SYNC_SAMPLES && now() - start >= CLOCK_SYNC_BUDGET_MS) {
      return;
    }
  }
}
