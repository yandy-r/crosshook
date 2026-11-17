import { callCommand } from '@/lib/ipc';

/** Phases the backend accepts on `bench_mark`; mirrors `crosshook_app::bench::mark::Phase`. */
export type BenchMarkPhase =
  | 'armed'
  | 'render'
  | 'focus_lost'
  | 'console_hidden'
  | 'follow_on'
  | 'follow_off'
  | 'health_ready'
  | 'health_rows_too_few';

let warned = false;

/** Warns at most once per page load (or since `resetBenchWarning`); bench probes never spam the console. */
export function warnOnce(message: string, error: unknown): void {
  if (!warned) {
    warned = true;
    console.warn(message, error);
  }
}

/** Re-arms `warnOnce`. Tests only: production keeps the page-load-wide warn-once. */
export function resetBenchWarning(): void {
  warned = false;
}

/**
 * Sends one `bench_mark`. Never rejects: a failure warns once and resolves `false`, so callers
 * can stop without a try/catch.
 */
export async function sendMark(
  scenario: string,
  seq: number,
  phase: BenchMarkPhase,
  tPerfMs: number
): Promise<boolean> {
  try {
    await callCommand('bench_mark', { scenario, seq, phase, tPerfMs });
    return true;
  } catch (error) {
    warnOnce('bench_mark call failed:', error);
    return false;
  }
}
