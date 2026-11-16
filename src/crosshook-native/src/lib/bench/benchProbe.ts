import { callCommand } from '@/lib/ipc';
import { getScenario, type ScenarioTrial } from './benchScenarios';
import { runClockSync } from './clockSync';

/** Upper bound on trials per probe run; the runner stops the app long before this. */
export const TRIAL_CAP = 10_000;

export type BenchMarkPhase = 'armed' | 'render' | 'focus_lost';

export interface BenchProbeOptions {
  /** Stops the probe and removes its listeners (tests; production never passes one). */
  signal?: AbortSignal;
  /** Overrides `TRIAL_CAP` (tests). */
  maxTrials?: number;
}

let warned = false;

function warnOnce(message: string, error: unknown): void {
  if (!warned) {
    warned = true;
    console.warn(message, error);
  }
}

async function sendMark(scenario: string, seq: number, phase: BenchMarkPhase, tPerfMs: number): Promise<boolean> {
  try {
    await callCommand('bench_mark', { scenario, seq, phase, tPerfMs });
    return true;
  } catch (error) {
    warnOnce('bench_mark call failed:', error);
    return false;
  }
}

function nextFrame(): Promise<void> {
  return new Promise((resolve) => requestAnimationFrame(() => resolve()));
}

/**
 * Bench-only probe, started when `bench_ready` returned a scenario. Syncs the
 * clock, calls `bench_begin`, then per trial: focuses the scenario target,
 * sends `armed`, waits for the DOM change, and sends `render` after the next
 * animation frame with the `performance.now()` read at the change. The runner
 * injects the input; the probe never does. Never rejects.
 */
export async function startBenchProbe(name: string, options: BenchProbeOptions = {}): Promise<void> {
  const scenario = getScenario(name);
  if (!scenario) {
    console.warn('bench probe: unsupported scenario', name);
    return;
  }
  const { signal, maxTrials = TRIAL_CAP } = options;

  let seq = 0;
  let trial: ScenarioTrial | null = null;
  const onBlur = () => {
    void sendMark(name, seq, 'focus_lost', performance.now());
  };
  window.addEventListener('blur', onBlur);

  const aborted = new Promise<null>((resolve) => {
    if (signal?.aborted) {
      resolve(null);
    } else {
      signal?.addEventListener('abort', () => resolve(null), { once: true });
    }
  });

  try {
    await runClockSync();
    await callCommand('bench_begin');
    // A setup or target failure sends no mark: the runner then reports MARK_TIMEOUT for the
    // missing `armed`, and `focus_lost` stays reserved for a real window blur.
    if (scenario.prepare && !(await scenario.prepare())) {
      return;
    }

    for (seq = 0; seq < maxTrials && !signal?.aborted; seq += 1) {
      trial = scenario.arm();
      if (!trial) {
        return;
      }
      if (!(await sendMark(name, seq, 'armed', performance.now()))) {
        return;
      }
      const observedAt = await Promise.race([trial.changed, aborted]);
      trial.dispose();
      trial = null;
      if (observedAt === null) {
        return;
      }
      await nextFrame();
      if (!(await sendMark(name, seq, 'render', observedAt))) {
        return;
      }
    }
  } catch (error) {
    warnOnce('bench probe failed:', error);
  } finally {
    trial?.dispose();
    window.removeEventListener('blur', onBlur);
  }
}
