import type { BreakpointSize } from '@/hooks/useBreakpoint';

export const INSPECTOR_WIDTHS: Record<BreakpointSize, number> = {
  uw: 360,
  desk: 320,
  narrow: 280,
  deck: 0,
} as const;

export function inspectorWidthForBreakpoint(size: BreakpointSize): number {
  return INSPECTOR_WIDTHS[size];
}
