import { isTauri } from '../runtime';
import type { OpenDialogOptions, SaveDialogOptions } from '@tauri-apps/plugin-dialog';

export type { OpenDialogOptions, SaveDialogOptions };

/**
 * Open a file/directory selection dialog.
 * In Tauri mode, delegates to the real @tauri-apps/plugin-dialog.open().
 * In browser mode, returns null (mimicking user cancellation) and emits a [dev-mock] warning.
 */
export async function open(options?: OpenDialogOptions): Promise<string | string[] | null> {
  if (isTauri()) {
    const real = await import('@tauri-apps/plugin-dialog');
    return real.open(options);
  }
  console.warn('[dev-mock] dialog.open suppressed in browser mode — call ignored');
  return null;
}

/**
 * Open a file save dialog.
 * In Tauri mode, delegates to the real @tauri-apps/plugin-dialog.save().
 * In browser mode, returns null (mimicking user cancellation) and emits a [dev-mock] warning.
 */
export async function save(options?: SaveDialogOptions): Promise<string | null> {
  if (isTauri()) {
    const real = await import('@tauri-apps/plugin-dialog');
    return real.save(options);
  }
  console.warn('[dev-mock] dialog.save suppressed in browser mode — call ignored');
  return null;
}
