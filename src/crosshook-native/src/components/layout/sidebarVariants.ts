import type { BreakpointSize } from '@/hooks/useBreakpoint';

export type SidebarVariant = 'rail' | 'mid' | 'full';

export const SIDEBAR_VARIANT_WIDTHS = {
  rail: 56,
  mid: 68,
  full: 240,
} as const satisfies Record<SidebarVariant, number>;

export function sidebarVariantFromBreakpoint(size: BreakpointSize, height = 900): SidebarVariant {
  switch (size) {
    case 'deck':
      return 'rail';
    case 'narrow':
      return height <= 820 ? 'rail' : 'mid';
    case 'desk':
    case 'uw':
      return 'full';
  }
}

export function sidebarWidthForVariant(variant: SidebarVariant): number {
  return SIDEBAR_VARIANT_WIDTHS[variant];
}

export function isSidebarCollapsedVariant(variant: SidebarVariant): boolean {
  return variant !== 'full';
}
