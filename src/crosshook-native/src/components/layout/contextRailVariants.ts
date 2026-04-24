import type { AppRoute } from '@/components/layout/Sidebar';
import type { LibraryShellMode } from '@/context/InspectorSelectionContext';
import type { BreakpointSize } from '@/hooks/useBreakpoint';

export type { LibraryShellMode };

export interface ContextRailLayout {
  visible: boolean;
  width: number;
}

/**
 * Rail width when visible (fourth pane beside inspector).
 */
export const CONTEXT_RAIL_WIDTH_PX = 300;

/**
 * Ultrawide viewport gate for the context rail (product acceptance: 3440×1440 shows,
 * 2560×1440 hides). Both widths still map to `uw` in `useBreakpoint` (≥2200), so this
 * intentionally uses raw element width instead of redefining global breakpoint buckets.
 * Set to 3300 (not 3440) because the AppShell element is ~64px narrower than the viewport
 * due to `padding: 0 32px` on the main element — at 3440px viewport, element width ≈ 3376.
 */
export const CONTEXT_RAIL_MIN_VIEWPORT_WIDTH = 3300;

export function contextRailLayoutForShell(input: {
  route: AppRoute;
  libraryMode: LibraryShellMode;
  /** Breakpoint bucket — retained for API symmetry / future use; rail gating uses viewport width. */
  breakpointSize: BreakpointSize;
  viewportWidth: number;
  viewportHeight: number;
}): ContextRailLayout {
  void input.breakpointSize;
  void input.viewportHeight;

  if (input.route !== 'library' || input.libraryMode === 'detail') {
    return { visible: false, width: 0 };
  }

  if (input.viewportWidth < CONTEXT_RAIL_MIN_VIEWPORT_WIDTH) {
    return { visible: false, width: 0 };
  }

  return { visible: true, width: CONTEXT_RAIL_WIDTH_PX };
}
