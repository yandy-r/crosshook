import { type RefObject, useCallback, useLayoutEffect, useRef, useState } from 'react';

/** Pixel thresholds: uw ≥ 2200 · desk ≥ 1440 · narrow ≥ 1100 · deck &lt; 1100 */
export const BREAKPOINTS = { narrow: 1100, desk: 1440, uw: 2200 } as const;

export type BreakpointSize = 'uw' | 'desk' | 'narrow' | 'deck';

export type UseBreakpointResult = {
  size: BreakpointSize;
  width: number;
  height: number;
  isDeck: boolean;
  isNarrow: boolean;
  isDesk: boolean;
  isUw: boolean;
};

const FALLBACK_NO_WINDOW: UseBreakpointResult = {
  size: 'desk',
  width: 1440,
  height: 900,
  isDeck: false,
  isNarrow: false,
  isDesk: true,
  isUw: false,
};

export function sizeFromWidth(width: number): BreakpointSize {
  if (width >= BREAKPOINTS.uw) {
    return 'uw';
  }
  if (width >= BREAKPOINTS.desk) {
    return 'desk';
  }
  if (width >= BREAKPOINTS.narrow) {
    return 'narrow';
  }
  return 'deck';
}

function deriveState(width: number, height: number): UseBreakpointResult {
  const size = sizeFromWidth(width);
  return {
    size,
    width,
    height,
    isDeck: size === 'deck',
    isNarrow: size === 'narrow',
    isDesk: size === 'desk',
    isUw: size === 'uw',
  };
}

function readWidthHeight(shellRef: RefObject<HTMLElement | null> | undefined): { width: number; height: number } {
  if (typeof window === 'undefined') {
    return { width: FALLBACK_NO_WINDOW.width, height: FALLBACK_NO_WINDOW.height };
  }
  const el = shellRef?.current;
  if (el) {
    const rect = el.getBoundingClientRect();
    return { width: Math.round(rect.width), height: Math.round(rect.height) };
  }
  return { width: window.innerWidth, height: window.innerHeight };
}

const WINDOW_BREAKPOINT_QUERIES = ['(min-width: 2200px)', '(min-width: 1440px)', '(min-width: 1100px)'] as const;

/**
 * Viewport- or element-aware breakpoint with `uw / desk / narrow / deck` buckets.
 * When `shellRef` is set, `ResizeObserver` uses that element; otherwise the window
 * and `matchMedia` listeners drive updates. Debounced with `requestAnimationFrame`.
 */
export function useBreakpoint(shellRef?: RefObject<HTMLElement | null>): UseBreakpointResult {
  const [state, setState] = useState<UseBreakpointResult>(() => {
    if (typeof window === 'undefined' || !window.matchMedia) {
      return FALLBACK_NO_WINDOW;
    }
    const { width, height } = readWidthHeight(shellRef);
    return deriveState(width, height);
  });

  const rafIdRef = useRef(0);
  const schedule = useCallback(() => {
    if (typeof window === 'undefined' || !window.matchMedia) {
      return;
    }
    cancelAnimationFrame(rafIdRef.current);
    rafIdRef.current = requestAnimationFrame(() => {
      const { width, height } = readWidthHeight(shellRef);
      setState(deriveState(width, height));
    });
  }, [shellRef]);

  useLayoutEffect(() => {
    if (typeof window === 'undefined' || !window.matchMedia) {
      return;
    }

    schedule();

    const cleanups: (() => void)[] = [];

    const el = shellRef?.current ?? null;
    if (el && typeof ResizeObserver !== 'undefined') {
      const ro = new ResizeObserver(() => {
        schedule();
      });
      ro.observe(el);
      cleanups.push(() => {
        ro.disconnect();
      });
    } else {
      for (const query of WINDOW_BREAKPOINT_QUERIES) {
        const mql = window.matchMedia(query);
        const onChange = (): void => {
          schedule();
        };
        mql.addEventListener('change', onChange);
        if (typeof mql.addListener === 'function') {
          mql.addListener(onChange);
        }
        cleanups.push(() => {
          mql.removeEventListener('change', onChange);
          if (typeof mql.removeListener === 'function') {
            mql.removeListener(onChange);
          }
        });
      }
    }

    return () => {
      for (const c of cleanups) {
        c();
      }
      cancelAnimationFrame(rafIdRef.current);
    };
  }, [shellRef, schedule]);

  if (typeof window === 'undefined' || !window.matchMedia) {
    return FALLBACK_NO_WINDOW;
  }

  return state;
}
