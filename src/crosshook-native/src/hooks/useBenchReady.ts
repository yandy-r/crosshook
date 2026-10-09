import { useEffect } from 'react';
import { callCommand } from '@/lib/ipc';

// Module-level one-shots: fire bench_ready at most once per process, even under
// StrictMode double-mounts and page remounts. Warn at most once on rejection.
let fired = false;
let warned = false;

/**
 * Startup-benchmark hook (YAN-782 / #509). When `ready` first becomes true,
 * waits two animation frames (so the ready paint is committed), then calls the
 * backend `bench_ready` command once per process. No-op outside benchmark mode
 * (the backend ignores the call unless CROSSHOOK_BENCH=1). Renders nothing.
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
        callCommand('bench_ready').catch((error: unknown) => {
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
