import { isTauri } from '../runtime';
import type { OpenDialogOptions, SaveDialogOptions } from '@tauri-apps/plugin-dialog';

export type { OpenDialogOptions, SaveDialogOptions };

/**
 * Open a file/directory selection dialog.
 * In Tauri mode, delegates to the real @tauri-apps/plugin-dialog.open().
 * In browser mode, returns null — native pickers are not used; call sites that need dev UX should branch on `isBrowserDevUi()` (see collection preset import/export).
 */
export async function open(options?: OpenDialogOptions): Promise<string | string[] | null> {
  if (isTauri()) {
    const real = await import('@tauri-apps/plugin-dialog');
    return real.open(options);
  }
  if (options?.directory) {
    console.warn('[plugin-stub] dialog.open directory suppressed in browser mode — call ignored');
  }
  return null;
}

/**
 * Open a file save dialog.
 * In Tauri mode, delegates to the real @tauri-apps/plugin-dialog.save().
 * In browser mode, returns null — use dev-only flows (e.g. preset export explainer) instead of prompts.
 */
export async function save(options?: SaveDialogOptions): Promise<string | null> {
  if (isTauri()) {
    const real = await import('@tauri-apps/plugin-dialog');
    return real.save(options);
  }
  return null;
}
