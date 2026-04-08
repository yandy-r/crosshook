/** Returns true when running inside a Tauri v2 WebView, false in any plain browser context. Single source of truth for runtime branching across the lib/ adapter layer. */
export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}
