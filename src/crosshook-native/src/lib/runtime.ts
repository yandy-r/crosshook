/** Returns true when running inside a Tauri v2 WebView, false in any plain browser context. Single source of truth for runtime branching across the lib/ adapter layer. */
export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

/** Plain browser (Vite dev / `--browser`): no native dialogs or Tauri IPC; use dev-only UX where needed. */
export function isBrowserDevUi(): boolean {
  return typeof window !== 'undefined' && !isTauri();
}
