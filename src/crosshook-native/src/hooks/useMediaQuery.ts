import { useEffect, useState } from 'react';

/**
 * Subscribes to a CSS media query. Returns `fallback` when `window.matchMedia`
 * is unavailable (degraded environments); guard style mirrors useBreakpoint.
 */
export function useMediaQuery(query: string, fallback = false): boolean {
  const [matches, setMatches] = useState<boolean>(() => {
    if (typeof window === 'undefined' || !window.matchMedia) {
      return fallback;
    }
    return window.matchMedia(query).matches;
  });

  useEffect(() => {
    if (typeof window === 'undefined' || !window.matchMedia) {
      return;
    }
    const mql = window.matchMedia(query);
    setMatches(mql.matches);
    const onChange = (event: MediaQueryListEvent) => setMatches(event.matches);
    // Older WebKitGTK MediaQueryList lacks addEventListener; fall back to the
    // deprecated addListener/removeListener pair (same compat path as useBreakpoint).
    if (typeof mql.addEventListener === 'function') {
      mql.addEventListener('change', onChange);
      return () => mql.removeEventListener('change', onChange);
    }
    mql.addListener(onChange);
    return () => mql.removeListener(onChange);
  }, [query]);

  return matches;
}
