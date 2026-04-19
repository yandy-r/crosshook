import type { InvokeArgs } from '@tauri-apps/api/core';
import { type RenderOptions, type RenderResult, render } from '@testing-library/react';
import type { ReactElement } from 'react';
import { type Handler, registerMocks, resetMockEnvironment } from '@/lib/mocks';

export interface MockRenderOptions extends Omit<RenderOptions, 'queries'> {
  seed?: (handlers: Map<string, Handler>) => void;
  handlerOverrides?: Record<string, Handler>;
}

let activeHandlers: Map<string, Handler> | null = null;

function createHandlerMap(options: Pick<MockRenderOptions, 'seed' | 'handlerOverrides'> = {}): Map<string, Handler> {
  resetMockEnvironment();
  const handlers = registerMocks();
  options.seed?.(handlers);
  for (const [name, handler] of Object.entries(options.handlerOverrides ?? {})) {
    handlers.set(name, handler);
  }
  activeHandlers = handlers;
  return handlers;
}

export function configureMockHandlers(
  options: Pick<MockRenderOptions, 'seed' | 'handlerOverrides'> = {}
): Map<string, Handler> {
  return createHandlerMap(options);
}

export function resetMockHandlers(): void {
  activeHandlers = null;
  resetMockEnvironment();
}

export async function mockCallCommand<T>(name: string, args?: InvokeArgs): Promise<T> {
  const handlers = activeHandlers ?? createHandlerMap();
  const handler = handlers.get(name);
  if (!handler) {
    throw new Error(`[test-mock] Unhandled command: ${name}`);
  }
  return (await handler(args ?? {})) as T;
}

export function renderWithMocks(ui: ReactElement, options: MockRenderOptions = {}): RenderResult {
  const { seed, handlerOverrides, ...renderOptions } = options;
  createHandlerMap({ seed, handlerOverrides });
  return render(ui, renderOptions);
}
