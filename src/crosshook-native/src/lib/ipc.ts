// src/crosshook-native/src/lib/ipc.ts
import type { InvokeArgs } from '@tauri-apps/api/core';
import { isTauri } from './runtime';

declare const __WEB_DEV_MODE__: boolean;

export async function callCommand<T>(name: string, args?: InvokeArgs): Promise<T> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<T>(name, args);
  }
  if (__WEB_DEV_MODE__) {
    const { runMockCommand } = await import('./ipc.dev');
    return runMockCommand<T>(name, args);
  }
  throw new Error('CrossHook commands require the Tauri desktop app or a webdev dev-server build.');
}
