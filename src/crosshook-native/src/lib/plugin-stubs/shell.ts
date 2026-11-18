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
