import { act, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  FORCED_COLORS_QUERY,
  PREFERS_CONTRAST_QUERY,
  resolveHighContrast,
  resolveMotion,
  useAriaLabelAudit,
  useHighContrastTheme,
  useMotionAttributeSync,
} from '@/hooks/useAccessibilityPreferences';
import { useMediaQuery } from '@/hooks/useMediaQuery';
import { MOTION_ATTRIBUTE, REDUCED_MOTION_QUERY } from '@/lib/motion';
import type { HighContrastPreference, ReducedMotionPreference } from '@/types/settings';

type MediaQueryListener = (event: MediaQueryListEvent) => void;

/**
 * Per-query matchMedia stub. The global mock in test/setup.ts always returns
 * `matches: false`; this helper lets each test control individual queries and
 * fire live change events (pattern from useBreakpoint.test.tsx).
 */
function stubMatchMedia(matchesByQuery: Record<string, boolean>): {
  fire(query: string, matches: boolean): void;
  restore(): void;
} {
  const state = new Map(Object.entries(matchesByQuery));
  const listeners = new Map<string, Set<MediaQueryListener>>();
  const original = Object.getOwnPropertyDescriptor(window, 'matchMedia');

  const register = (query: string, listener: MediaQueryListener): void => {
    const set = listeners.get(query) ?? new Set();
    set.add(listener);
    listeners.set(query, set);
  };
  const unregister = (query: string, listener: MediaQueryListener): void => {
    listeners.get(query)?.delete(listener);
  };

  Object.defineProperty(window, 'matchMedia', {
    configurable: true,
    writable: true,
    value: (query: string) => ({
      get matches() {
        return state.get(query) ?? false;
      },
      media: query,
      onchange: null,
      addListener: (listener: MediaQueryListener) => register(query, listener),
      removeListener: (listener: MediaQueryListener) => unregister(query, listener),
      addEventListener: (_type: string, listener: MediaQueryListener) => register(query, listener),
      removeEventListener: (_type: string, listener: MediaQueryListener) => unregister(query, listener),
      dispatchEvent: () => true,
    }),
  });

  return {
    fire(query: string, matches: boolean) {
      state.set(query, matches);
      for (const listener of listeners.get(query) ?? []) {
        listener({ matches, media: query } as MediaQueryListEvent);
      }
    },
    restore() {
      if (original) {
        Object.defineProperty(window, 'matchMedia', original);
      }
    },
  };
}

function undefineMatchMedia(): () => void {
  const original = Object.getOwnPropertyDescriptor(window, 'matchMedia');
  Object.defineProperty(window, 'matchMedia', {
    configurable: true,
    writable: true,
    value: undefined,
  });
  return () => {
    if (original) {
      Object.defineProperty(window, 'matchMedia', original);
    }
  };
}

afterEach(() => {
  document.documentElement.removeAttribute('data-crosshook-theme');
  document.documentElement.removeAttribute(MOTION_ATTRIBUTE);
});

describe('resolveHighContrast', () => {
  it('resolveHighContrast full matrix', () => {
    const flags = [false, true];
    for (const prefersMore of flags) {
      for (const forcedColors of flags) {
        expect(resolveHighContrast('on', prefersMore, forcedColors)).toBe(true);
        expect(resolveHighContrast('off', prefersMore, forcedColors)).toBe(false);
        expect(resolveHighContrast('auto', prefersMore, forcedColors)).toBe(prefersMore || forcedColors);
      }
    }
  });
});

describe('resolveMotion', () => {
  it('resolveMotion full matrix', () => {
    for (const prefersReduced of [false, true]) {
      expect(resolveMotion('reduced', prefersReduced)).toBe('reduced');
      expect(resolveMotion('full', prefersReduced)).toBe('full');
      expect(resolveMotion('auto', prefersReduced)).toBe(prefersReduced ? 'reduced' : 'full');
    }
  });
});

describe('useHighContrastTheme', () => {
  it('useHighContrastTheme sets theme attribute for manual on', () => {
    const stub = stubMatchMedia({});
    try {
      renderHook(() => useHighContrastTheme('on'));
      expect(document.documentElement.getAttribute('data-crosshook-theme')).toBe('high-contrast');
    } finally {
      stub.restore();
    }
  });

  it('useHighContrastTheme manual off beats prefers-contrast', () => {
    const stub = stubMatchMedia({ [PREFERS_CONTRAST_QUERY]: true });
    try {
      renderHook(() => useHighContrastTheme('off'));
      expect(document.documentElement.getAttribute('data-crosshook-theme')).toBeNull();
    } finally {
      stub.restore();
    }
  });

  it('useHighContrastTheme auto follows forced-colors', () => {
    const stub = stubMatchMedia({ [FORCED_COLORS_QUERY]: true });
    try {
      renderHook(() => useHighContrastTheme('auto'));
      expect(document.documentElement.getAttribute('data-crosshook-theme')).toBe('high-contrast');
    } finally {
      stub.restore();
    }
  });

  it('useHighContrastTheme auto flips on live change event', () => {
    const stub = stubMatchMedia({});
    try {
      renderHook(() => useHighContrastTheme('auto'));
      expect(document.documentElement.getAttribute('data-crosshook-theme')).toBeNull();

      act(() => {
        stub.fire(PREFERS_CONTRAST_QUERY, true);
      });
      expect(document.documentElement.getAttribute('data-crosshook-theme')).toBe('high-contrast');
    } finally {
      stub.restore();
    }
  });
});

describe('useMotionAttributeSync', () => {
  it('useMotionAttributeSync writes resolved attribute', () => {
    const stub = stubMatchMedia({});
    try {
      const { rerender } = renderHook(
        ({ setting }: { setting: ReducedMotionPreference }) => useMotionAttributeSync(setting),
        {
          initialProps: { setting: 'reduced' as ReducedMotionPreference },
        }
      );
      expect(document.documentElement.getAttribute(MOTION_ATTRIBUTE)).toBe('reduced');

      rerender({ setting: 'full' });
      expect(document.documentElement.getAttribute(MOTION_ATTRIBUTE)).toBe('full');

      act(() => {
        stub.fire(REDUCED_MOTION_QUERY, true);
      });
      rerender({ setting: 'auto' });
      expect(document.documentElement.getAttribute(MOTION_ATTRIBUTE)).toBe('reduced');
    } finally {
      stub.restore();
    }
  });

  it('manual full beats OS reduce', () => {
    const stub = stubMatchMedia({ [REDUCED_MOTION_QUERY]: true });
    try {
      renderHook(() => useMotionAttributeSync('full'));
      expect(document.documentElement.getAttribute(MOTION_ATTRIBUTE)).toBe('full');
    } finally {
      stub.restore();
    }
  });

  it('matchMedia absent degrades to contrast off and motion full', () => {
    const restore = undefineMatchMedia();
    try {
      renderHook(({ setting }: { setting: HighContrastPreference }) => useHighContrastTheme(setting), {
        initialProps: { setting: 'auto' as HighContrastPreference },
      });
      expect(document.documentElement.getAttribute('data-crosshook-theme')).toBeNull();

      renderHook(() => useMotionAttributeSync('auto'));
      expect(document.documentElement.getAttribute(MOTION_ATTRIBUTE)).toBe('full');
    } finally {
      restore();
    }
  });
});

describe('useMediaQuery', () => {
  it('useMediaQuery returns fallback without matchMedia', () => {
    const restore = undefineMatchMedia();
    try {
      const { result } = renderHook(() => useMediaQuery('(prefers-contrast: more)', true));
      expect(result.current).toBe(true);

      const { result: defaulted } = renderHook(() => useMediaQuery('(prefers-contrast: more)'));
      expect(defaulted.current).toBe(false);
    } finally {
      restore();
    }
  });

  it('useMediaQuery falls back to addListener when addEventListener is missing (legacy WebKitGTK)', () => {
    const original = Object.getOwnPropertyDescriptor(window, 'matchMedia');
    let matches = false;
    const listeners = new Set<MediaQueryListener>();
    // Legacy WebKitGTK MediaQueryList exposes only addListener/removeListener.
    const legacyMql = {
      get matches() {
        return matches;
      },
      media: '(prefers-contrast: more)',
      onchange: null,
      addListener: (listener: MediaQueryListener) => listeners.add(listener),
      removeListener: (listener: MediaQueryListener) => listeners.delete(listener),
    };
    Object.defineProperty(window, 'matchMedia', {
      configurable: true,
      writable: true,
      value: () => legacyMql as unknown as MediaQueryList,
    });

    try {
      const { result, unmount } = renderHook(() => useMediaQuery('(prefers-contrast: more)'));
      expect(result.current).toBe(false);
      expect(listeners.size).toBe(1);

      act(() => {
        matches = true;
        for (const listener of listeners) {
          listener({ matches: true, media: '(prefers-contrast: more)' } as MediaQueryListEvent);
        }
      });
      expect(result.current).toBe(true);

      unmount();
      expect(listeners.size).toBe(0);
    } finally {
      if (original) {
        Object.defineProperty(window, 'matchMedia', original);
      }
    }
  });
});

describe('useAriaLabelAudit', () => {
  it('useAriaLabelAudit warns for unnamed buttons and does not mutate the DOM', async () => {
    const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {});
    try {
      renderHook(() => useAriaLabelAudit());
      warnSpy.mockClear();

      const unnamed = document.createElement('button');
      unnamed.title = 'x';
      document.body.append(unnamed);

      const named = document.createElement('button');
      named.textContent = 'Launch';
      document.body.append(named);

      await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 0));
      });

      expect(warnSpy).toHaveBeenCalledTimes(1);
      expect(String(warnSpy.mock.calls[0]?.[1])).toContain('<button');
      expect(unnamed.hasAttribute('aria-label')).toBe(false);

      unnamed.remove();
      named.remove();
    } finally {
      warnSpy.mockRestore();
    }
  });
});
