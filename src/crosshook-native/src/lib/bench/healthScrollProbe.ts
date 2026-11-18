import { callCommand } from '@/lib/ipc';
import { warnOnce } from './benchMark';
import { HEALTH_WAIT_MS, LIVE_TABLE, reportHealthTable } from './healthTable';

/** Scenario name the runner passes for the G4 health-table scroll. */
export const HEALTH_SCROLL_SCENARIO = 'health_scroll';

interface HealthScrollProbeOptions {
  /** Stops the wait (tests; production never passes one). */
  signal?: AbortSignal;
  /** Overrides `HEALTH_WAIT_MS` (tests). */
  timeoutMs?: number;
}

/**
 * Bench-only probe for the G4 health scroll. Does no clock sync. Opens the health page and waits
 * for the live table; with too few rows it reports `health_rows_too_few` and never calls
 * `bench_begin`. Otherwise it reports `health_ready`, scrolls the table to the top of the content
 * root, and calls `bench_begin`; the runner then waits for the page to go idle and drives the
 * wheel. Never rejects.
 */
export async function startHealthScrollProbe(options: HealthScrollProbeOptions = {}): Promise<void> {
  const { signal, timeoutMs = HEALTH_WAIT_MS } = options;
  try {
    if (!(await reportHealthTable(HEALTH_SCROLL_SCENARIO, signal, timeoutMs)) || signal?.aborted) {
      return;
    }
    // `behavior: 'auto'` is an instant jump, not an animation, so `motion.ts` does not apply.
    document.querySelector(LIVE_TABLE)?.scrollIntoView({ block: 'start', behavior: 'auto' });
    await callCommand('bench_begin');
  } catch (error) {
    warnOnce('bench health scroll probe failed:', error);
  }
}
