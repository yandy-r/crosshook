import '@testing-library/jest-dom/vitest';
import { webcrypto } from 'node:crypto';
import { cleanup } from '@testing-library/react';
import { afterEach, vi } from 'vitest';
import { resetMockHandlers } from './render';

type MatchMediaResult = {
  matches: boolean;
  media: string;
  onchange: ((event: MediaQueryListEvent) => void) | null;
  addListener: (listener: (event: MediaQueryListEvent) => void) => void;
  removeListener: (listener: (event: MediaQueryListEvent) => void) => void;
  addEventListener: (type: string, listener: (event: MediaQueryListEvent) => void) => void;
  removeEventListener: (type: string, listener: (event: MediaQueryListEvent) => void) => void;
  dispatchEvent: (event: Event) => boolean;
};

declare global {
  interface Window {
    IntersectionObserver: typeof MockIntersectionObserver;
    ResizeObserver: typeof MockResizeObserver;
  }
}

const matchMediaListeners = new Map<string, Set<(event: MediaQueryListEvent) => void>>();
let nextAnimationFrameId = 1;
const animationFrameHandles = new Map<number, number>();

class MockIntersectionObserver implements IntersectionObserver {
  static instances: MockIntersectionObserver[] = [];

  readonly root = null;
  readonly rootMargin = '';
  readonly thresholds = [0];
  private readonly callback: IntersectionObserverCallback;
  private observedTargets = new Set<Element>();

  constructor(callback: IntersectionObserverCallback) {
    this.callback = callback;
    MockIntersectionObserver.instances.push(this);
  }

  disconnect(): void {
    this.observedTargets.clear();
  }

  observe(target: Element): void {
    this.observedTargets.add(target);
  }

  takeRecords(): IntersectionObserverEntry[] {
    return [];
  }

  unobserve(target: Element): void {
    this.observedTargets.delete(target);
  }

  trigger(target: Element, isIntersecting = true): void {
    if (!this.observedTargets.has(target)) {
      return;
    }
    const rect = target.getBoundingClientRect();
    this.callback(
      [
        {
          boundingClientRect: rect,
          intersectionRatio: isIntersecting ? 1 : 0,
          intersectionRect: isIntersecting ? rect : new DOMRectReadOnly(),
          isIntersecting,
          rootBounds: null,
          target,
          time: Date.now(),
        },
      ],
      this
    );
  }

  static reset(): void {
    MockIntersectionObserver.instances = [];
  }
}

class MockResizeObserver implements ResizeObserver {
  static instances: MockResizeObserver[] = [];

  private readonly callback: ResizeObserverCallback;
  private readonly observedTargets = new Set<Element>();

  constructor(callback: ResizeObserverCallback) {
    this.callback = callback;
    MockResizeObserver.instances.push(this);
  }

  disconnect(): void {
    this.observedTargets.clear();
  }

  observe(target: Element): void {
    this.observedTargets.add(target);
  }

  unobserve(target: Element): void {
    this.observedTargets.delete(target);
  }

  trigger(target: Element, width: number, height: number): void {
    if (!this.observedTargets.has(target)) {
      return;
    }
    const contentRect = new DOMRectReadOnly(0, 0, width, height);
    const block = { inlineSize: width, blockSize: height };
    this.callback(
      [
        {
          target,
          contentRect,
          contentBoxSize: [block],
          borderBoxSize: [block],
          devicePixelContentBoxSize: [block],
        } as unknown as ResizeObserverEntry,
      ],
      this
    );
  }

  static reset(): void {
    MockResizeObserver.instances = [];
  }
}

function createChangeEvent(query: string): MediaQueryListEvent {
  if (typeof MediaQueryListEvent !== 'undefined') {
    return new MediaQueryListEvent('change', { media: query, matches: true });
  }
  return { type: 'change', media: query, matches: true, bubbles: false } as MediaQueryListEvent;
}

/**
 * Invokes all listeners registered for `query` (via the test `matchMedia` mock).
 * Use after changing `innerWidth` / `innerHeight` so `useBreakpoint`’s
 * `schedule()` reads the new dimensions.
 */
export function fireMatchMediaChangeListeners(query: string): void {
  const set = matchMediaListeners.get(query);
  if (set === undefined) {
    return;
  }
  const event = createChangeEvent(query);
  for (const fn of set) {
    fn(event);
  }
}

if (!globalThis.crypto) {
  Object.defineProperty(globalThis, 'crypto', {
    value: webcrypto,
    configurable: true,
  });
}

if (!window.matchMedia) {
  Object.defineProperty(window, 'matchMedia', {
    writable: true,
    value: (query: string): MatchMediaResult => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: (listener: (event: MediaQueryListEvent) => void) => {
        const listeners = matchMediaListeners.get(query) ?? new Set();
        listeners.add(listener);
        matchMediaListeners.set(query, listeners);
      },
      removeListener: (listener: (event: MediaQueryListEvent) => void) => {
        matchMediaListeners.get(query)?.delete(listener);
      },
      addEventListener: (_type: string, listener: (event: MediaQueryListEvent) => void) => {
        const listeners = matchMediaListeners.get(query) ?? new Set();
        listeners.add(listener);
        matchMediaListeners.set(query, listeners);
      },
      removeEventListener: (_type: string, listener: (event: MediaQueryListEvent) => void) => {
        matchMediaListeners.get(query)?.delete(listener);
      },
      dispatchEvent: () => true,
    }),
  });
}

Object.defineProperty(window, 'IntersectionObserver', {
  writable: true,
  configurable: true,
  value: MockIntersectionObserver,
});

Object.defineProperty(window, 'ResizeObserver', {
  writable: true,
  configurable: true,
  value: MockResizeObserver,
});

Object.defineProperty(window.HTMLElement.prototype, 'scrollIntoView', {
  configurable: true,
  value: vi.fn(),
});

if (!navigator.getGamepads) {
  Object.defineProperty(navigator, 'getGamepads', {
    configurable: true,
    value: () => [],
  });
}

window.requestAnimationFrame = (callback: FrameRequestCallback): number => {
  const id = nextAnimationFrameId++;
  const handle = window.setTimeout(() => {
    animationFrameHandles.delete(id);
    callback(Date.now());
  }, 16);
  animationFrameHandles.set(id, handle);
  return id;
};

window.cancelAnimationFrame = (id: number): void => {
  const handle = animationFrameHandles.get(id);
  if (handle !== undefined) {
    window.clearTimeout(handle);
    animationFrameHandles.delete(id);
  }
};

afterEach(() => {
  cleanup();
  resetMockHandlers();
  MockIntersectionObserver.reset();
  MockResizeObserver.reset();
  matchMediaListeners.clear();
  for (const handle of animationFrameHandles.values()) {
    window.clearTimeout(handle);
  }
  animationFrameHandles.clear();
  document.documentElement.removeAttribute('data-crosshook-controller-mode');
  vi.clearAllMocks();
  vi.useRealTimers();
});

export function triggerIntersection(target: Element, isIntersecting = true): void {
  for (const observer of MockIntersectionObserver.instances) {
    observer.trigger(target, isIntersecting);
  }
}

export function triggerResize(target: Element, width: number, height: number): void {
  for (const observer of MockResizeObserver.instances) {
    observer.trigger(target, width, height);
  }
}

export function resetMockResizeObserver(): void {
  MockResizeObserver.reset();
}

export { MockResizeObserver };

import { configureAxe, toHaveNoViolations } from 'jest-axe';
import { expect } from 'vitest';

expect.extend(toHaveNoViolations);

// Color contrast requires real CSS rendering; not meaningful in happy-dom.
configureAxe({
  rules: { 'color-contrast': { enabled: false } },
});
