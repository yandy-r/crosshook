import { convertFileSrc as realConvertFileSrc } from '@tauri-apps/api/core';
import { isTauri } from '../runtime';

/**
 * Synchronous shim for @tauri-apps/api/core convertFileSrc.
 *
 * Uses a static import (Strategy B) because this function is called from
 * useMemo and event handlers — it cannot be async-wrapped. The static import
 * keeps @tauri-apps/api/core in the browser-mode bundle, which is acceptable
 * because convertFileSrc is a tiny pure function with no side effects.
 *
 * In browser mode, returns the path unchanged. The resulting <img src> will
 * fail to load, falling back to the existing placeholder image rendering.
 */
export function convertFileSrc(path: string, protocol = 'asset'): string {
  if (isTauri()) {
    return realConvertFileSrc(path, protocol);
  }
  return path;
}
