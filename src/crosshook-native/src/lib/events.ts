import type { EventCallback, UnlistenFn } from '@tauri-apps/api/event';
import { isTauri } from './runtime';

type Listener = (payload: unknown) => void;
const browserBus = new Map<string, Set<Listener>>();

export async function subscribeEvent<T>(name: string, handler: EventCallback<T>): Promise<UnlistenFn> {
  if (isTauri()) {
    const { listen } = await import('@tauri-apps/api/event');
    return listen<T>(name, handler);
  }
  const wrapped: Listener = (payload) => handler({ event: name, id: 0, payload: payload as T });
  if (!browserBus.has(name)) browserBus.set(name, new Set());
  browserBus.get(name)!.add(wrapped);
  return () => {
    browserBus.get(name)?.delete(wrapped);
  };
}

/** Returns true if at least one browser-dev listener received the payload. */
export function emitMockEvent(name: string, payload: unknown): boolean {
  if (isTauri()) return false;
  const bus = browserBus.get(name);
  if (!bus || bus.size === 0) return false;
  for (const listener of bus) {
    listener(payload);
  }
  return true;
}
