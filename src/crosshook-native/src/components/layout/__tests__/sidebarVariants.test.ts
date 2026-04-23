import { describe, expect, it } from 'vitest';
import {
  isSidebarCollapsedVariant,
  SIDEBAR_VARIANT_WIDTHS,
  sidebarVariantFromBreakpoint,
  sidebarWidthForVariant,
} from '../sidebarVariants';

describe('sidebarVariants', () => {
  it('maps PRD breakpoint buckets to sidebar variants', () => {
    expect(sidebarVariantFromBreakpoint('deck')).toBe('rail');
    expect(sidebarVariantFromBreakpoint('narrow')).toBe('mid');
    expect(sidebarVariantFromBreakpoint('narrow', 800)).toBe('rail');
    expect(sidebarVariantFromBreakpoint('desk')).toBe('full');
    expect(sidebarVariantFromBreakpoint('uw')).toBe('full');
  });

  it('exposes the canonical width contract for each variant', () => {
    expect(sidebarWidthForVariant('rail')).toBe(56);
    expect(sidebarWidthForVariant('mid')).toBe(68);
    expect(sidebarWidthForVariant('full')).toBe(264);
    expect(SIDEBAR_VARIANT_WIDTHS).toEqual({ rail: 56, mid: 68, full: 264 });
  });

  it('treats non-full variants as collapsed', () => {
    expect(isSidebarCollapsedVariant('rail')).toBe(true);
    expect(isSidebarCollapsedVariant('mid')).toBe(true);
    expect(isSidebarCollapsedVariant('full')).toBe(false);
  });
});
