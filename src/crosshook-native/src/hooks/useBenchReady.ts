import { useEffect } from 'react';
import { callCommand } from '@/lib/ipc';

interface BenchReadyResult {
  name: string;
}

// Module-level one-shots: fire bench_ready at most once per process, even under
// StrictMode double-mounts and page remounts. Warn at most once on rejection.
let fired = false;
let warned = false;

/**
 * Startup-benchmark hook. When `ready` first becomes true,
 * waits two animation frames (so the ready paint is committed), then calls the
 * backend `bench_ready` command once per process. No-op outside benchmark mode
 * (the backend returns null unless CROSSHOOK_BENCH=1). When it returns a
 * scenario, lazily loads and starts the bench probe. Renders nothing.
 */
export function useBenchReady(ready: boolean): void {
  useEffect(() => {
    if (!ready || fired) {
      return;
    }
    let frameId = requestAnimationFrame(() => {
      frameId = requestAnimationFrame(() => {
        if (fired) {
          return;
        }
        fired = true;
        callCommand<BenchReadyResult | null | undefined>('bench_ready')
          .then(async (result) => {
            // Production (and every non-bench session) returns null: nothing else happens.
            if (typeof result?.name !== 'string') {
              return;
            }
            const { startBenchProbe } = await import('@/lib/bench/benchProbe');
            await startBenchProbe(result.name);
          })
          .catch((error: unknown) => {
            if (!warned) {
              warned = true;
              console.warn('bench_ready call failed:', error);
            }
          });
      });
    });
    return () => cancelAnimationFrame(frameId);
  }, [ready]);
}
