import { isTauri } from '../runtime';

/**
 * Open a URL in the system browser.
 * In Tauri mode, delegates to the real @tauri-apps/plugin-shell.open().
 * In browser mode, no-ops with a [dev-mock] warning — non-destructive operation per D4.
 */
export async function open(url: string): Promise<void> {
  if (isTauri()) {
    const real = await import('@tauri-apps/plugin-shell');
    return real.open(url);
  }
  console.warn('[dev-mock] shell.open suppressed in browser mode:', url);
}

/**
 * Stub for @tauri-apps/plugin-shell Command class.
 * CrossHook does not currently call Command in src/, but the type must exist so any
 * future Phase 2 import fails loudly rather than silently at runtime.
 */
export class Command {
  static create(): Command {
    throw new Error('[dev-mock] shell.Command is not available in browser dev mode');
  }

  spawn(): never {
    throw new Error('[dev-mock] shell.Command.spawn is not available in browser dev mode');
  }

  execute(): never {
    throw new Error('[dev-mock] shell.Command.execute is not available in browser dev mode');
  }
}
