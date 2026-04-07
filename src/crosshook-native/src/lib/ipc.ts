// src/crosshook-native/src/lib/ipc.ts
import type { InvokeArgs } from '@tauri-apps/api/core';
import { isTauri } from './runtime';

declare const __WEB_DEV_MODE__: boolean;

type Handler = (args: unknown) => unknown | Promise<unknown>;

/**
 * Promise latch — ensures a single concurrent import('./mocks') even when
 * multiple callCommand() calls fire in parallel on mount (e.g. PreferencesContext
 * issues 3 parallel commands). Without the latch each call would race to initiate
 * its own dynamic import and registerMocks() would run multiple times.
 */
let mocksPromise: Promise<Map<string, Handler>> | null = null;

async function ensureMocks(): Promise<Map<string, Handler>> {
  if (mocksPromise !== null) return mocksPromise;
  if (!__WEB_DEV_MODE__) {
    throw new Error(
      '[dev-mock] mock layer invoked in non-webdev build — check dev:browser script passes --mode webdev',
    );
  }
  // @vite-ignore prevents Vite from warning on the unresolvable specifier.
  // The mocks module is created by Task 1.12; this import intentionally fails
  // at runtime until that task lands (no callCommand() call sites exist yet).
  mocksPromise =
      (import(/* @vite-ignore */ './mocks') as Promise<{ registerMocks: () => Map<string, Handler> }>).then(
      (m) => m.registerMocks(),
    );
  return mocksPromise;
}

export async function callCommand<T>(name: string, args?: InvokeArgs): Promise<T> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<T>(name, args);
  }

  const map = await ensureMocks();
  const handler = map.get(name);
  if (!handler) {
    throw new Error(
      `[dev-mock] Unhandled command: ${name}. Add a handler in src/lib/mocks/handlers/<area>.ts — see lib/mocks/README.md`,
    );
  }
  if (import.meta.env.DEV) {
    console.debug('[mock] callCommand', name, args);
  }
  return handler(args ?? {}) as Promise<T>;
}
