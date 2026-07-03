/** DOM attribute carrying the resolved motion preference (owned by useMotionAttributeSync). */
export const MOTION_ATTRIBUTE = 'data-crosshook-motion';
export const REDUCED_MOTION_QUERY = '(prefers-reduced-motion: reduce)';

/**
 * Imperative resolved-motion read for code mounted outside PreferencesProvider
 * (useScrollEnhance, gamepad-nav). Attribute-first; matchMedia fallback covers
 * the pre-settings-load window; no matchMedia → full motion.
 */
export function isMotionReduced(): boolean {
  const value = document.documentElement.getAttribute(MOTION_ATTRIBUTE);
  if (value === 'reduced') return true;
  if (value === 'full') return false;
  return typeof window !== 'undefined' && !!window.matchMedia?.(REDUCED_MOTION_QUERY).matches;
}

export function scrollBehavior(): ScrollBehavior {
  return isMotionReduced() ? 'auto' : 'smooth';
}
