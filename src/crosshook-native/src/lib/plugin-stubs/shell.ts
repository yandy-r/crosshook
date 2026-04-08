import { isTauri } from '../runtime';

const isDev = import.meta.env.DEV;

/**
 * Open a URL in the system browser.
 * In Tauri mode, delegates to the real @tauri-apps/plugin-shell.open().
 * In browser dev mode, opens a new tab when possible.
 */
export async function open(url: string): Promise<void> {
  if (isTauri()) {
    const real = await import('@tauri-apps/plugin-shell');
    return real.open(url);
  }
  if (isDev) {
    console.warn('shell.open: using browser tab (dev mode):', url);
  }
  const opened = window.open(url, '_blank', 'noopener,noreferrer');
  if (opened === null && isDev) {
    console.warn('shell.open: window.open returned null (popup may be blocked)');
  }
}

/**
 * Stub for @tauri-apps/plugin-shell Command class.
 * CrossHook does not currently call Command in src/, but the type must exist so any
 * future Phase 2 import fails loudly rather than silently at runtime.
 */
export class Command {
  static create(): Command {
    throw new Error('shell.Command is not available outside the Tauri desktop app.');
  }

  spawn(): never {
    throw new Error('shell.Command.spawn is not available outside the Tauri desktop app.');
  }

  execute(): never {
    throw new Error('shell.Command.execute is not available outside the Tauri desktop app.');
  }
}
