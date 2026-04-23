import type { BreakpointSize, UseBreakpointResult } from '@/hooks/useBreakpoint';

/** Deterministic shell widths for unit tests (match the actual breakpoint buckets). */
export const BREAKPOINT_PX = { uw: 2560, desk: 1920, narrow: 1280, deck: 1024 } as const;

function flagsFor(size: BreakpointSize): Pick<UseBreakpointResult, 'isDeck' | 'isNarrow' | 'isDesk' | 'isUw'> {
  return {
    isDeck: size === 'deck',
    isNarrow: size === 'narrow',
    isDesk: size === 'desk',
    isUw: size === 'uw',
  };
}

/**
 * Pure stub result for `useBreakpoint` in unit tests.
 *
 * Usage (module top):
 * ```ts
 * import { useBreakpoint } from '@/hooks/useBreakpoint';
 * vi.mock('@/hooks/useBreakpoint', () => ({ useBreakpoint: vi.fn() }));
 * ```
 * Then in `beforeEach`:
 * `vi.mocked(useBreakpoint).mockReturnValue(breakpointResult('desk'));`
 */
export function breakpointResult(size: BreakpointSize, height = 800): UseBreakpointResult {
  return {
    size,
    width: BREAKPOINT_PX[size],
    height,
    ...flagsFor(size),
  };
}
