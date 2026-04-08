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

export function emitMockEvent(name: string, payload: unknown): void {
  if (isTauri()) return;
  const bus = browserBus.get(name);
  if (!bus) return;
  for (const listener of bus) {
    listener(payload);
  }
}
