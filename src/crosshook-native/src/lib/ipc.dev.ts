// Webdev-only mock IPC bridge. Bundled only when __WEB_DEV_MODE__ is true (see ipc.ts).
import type { InvokeArgs } from '@tauri-apps/api/core';

type Handler = (args: unknown) => unknown | Promise<unknown>;

/**
 * Promise latch — ensures a single concurrent import('./mocks') even when
 * multiple callCommand() calls fire in parallel on mount.
 */
let mocksPromise: Promise<Map<string, Handler>> | null = null;

async function ensureMocks(): Promise<Map<string, Handler>> {
  if (mocksPromise !== null) return mocksPromise;
  mocksPromise = (import(/* @vite-ignore */ './mocks') as Promise<{ registerMocks: () => Map<string, Handler> }>).then(
    (m) => m.registerMocks()
  );
  return mocksPromise;
}

export async function runMockCommand<T>(name: string, args?: InvokeArgs): Promise<T> {
  const map = await ensureMocks();
  const handler = map.get(name);
  if (!handler) {
    throw new Error(
      `[dev-mock] Unhandled command: ${name}. Add a handler in src/lib/mocks/handlers/<area>.ts — see lib/mocks/README.md`
    );
  }
  if (import.meta.env.DEV) {
    console.debug('[mock] callCommand', name, args);
  }
  return handler(args ?? {}) as Promise<T>;
}
