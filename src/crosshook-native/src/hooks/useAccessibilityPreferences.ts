// Resolution layer for the tri-state accessibility preferences: pure resolvers plus
// hooks that project the resolved state onto <html> attributes. Roving-tabindex
// follow-up adopters: the palette list and discovery results (future issues).
import { useEffect } from 'react';
import { MOTION_ATTRIBUTE, REDUCED_MOTION_QUERY } from '@/lib/motion';
import type { HighContrastPreference, ReducedMotionPreference } from '@/types/settings';
import { useMediaQuery } from './useMediaQuery';

export const PREFERS_CONTRAST_QUERY = '(prefers-contrast: more)';
export const FORCED_COLORS_QUERY = '(forced-colors: active)';
export const THEME_ATTRIBUTE = 'data-crosshook-theme';
export const HIGH_CONTRAST_THEME = 'high-contrast';

export function resolveHighContrast(
  setting: HighContrastPreference,
  prefersMore: boolean,
  forcedColors: boolean
): boolean {
  if (setting === 'on') return true;
  if (setting === 'off') return false;
  return prefersMore || forcedColors;
}

export function resolveMotion(setting: ReducedMotionPreference, prefersReduced: boolean): 'reduced' | 'full' {
  if (setting === 'reduced' || setting === 'full') return setting;
  return prefersReduced ? 'reduced' : 'full';
}

/** Resolves the tri-state high-contrast preference against the live media queries. */
export function useResolvedHighContrast(setting: HighContrastPreference): boolean {
  const prefersMore = useMediaQuery(PREFERS_CONTRAST_QUERY);
  const forcedColors = useMediaQuery(FORCED_COLORS_QUERY);
  return resolveHighContrast(setting, prefersMore, forcedColors);
}

/** Toggles `data-crosshook-theme="high-contrast"` on `<html>` from the resolved preference. */
export function useHighContrastTheme(setting: HighContrastPreference): void {
  const enabled = useResolvedHighContrast(setting);

  useEffect(() => {
    if (typeof document === 'undefined') return;
    const root = document.documentElement;
    if (enabled) {
      root.setAttribute(THEME_ATTRIBUTE, HIGH_CONTRAST_THEME);
    } else {
      root.removeAttribute(THEME_ATTRIBUTE);
    }

    return () => {
      root.removeAttribute(THEME_ATTRIBUTE);
    };
  }, [enabled]);
}

export function useReducedMotion(setting: ReducedMotionPreference): 'reduced' | 'full' {
  return resolveMotion(setting, useMediaQuery(REDUCED_MOTION_QUERY));
}

/**
 * Writes the resolved motion preference to `data-crosshook-motion` on `<html>`.
 * No cleanup — the attribute is app-global; `main.tsx` owns the initial value
 * and this hook owns it thereafter.
 */
export function useMotionAttributeSync(setting: ReducedMotionPreference): void {
  const motion = useReducedMotion(setting);

  useEffect(() => {
    if (typeof document === 'undefined') return;
    document.documentElement.setAttribute(MOTION_ATTRIBUTE, motion);
  }, [motion]);
}

const BUTTON_SELECTOR = 'button, [role="button"]';

function warnUnnamedButtons(root: ParentNode): void {
  // querySelectorAll only matches descendants — include the root when it is itself a button.
  const rootButton = root instanceof HTMLElement && root.matches(BUTTON_SELECTOR) ? [root] : [];
  for (const button of [...rootButton, ...root.querySelectorAll<HTMLElement>(BUTTON_SELECTOR)]) {
    if (!button.getAttribute('aria-label') && !button.getAttribute('aria-labelledby') && !button.textContent?.trim()) {
      console.warn('[a11y] button has no accessible name:', button.outerHTML.slice(0, 200));
    }
  }
}

/** Dev-only auditor: warns about buttons without accessible names; never mutates the DOM. */
export function useAriaLabelAudit(): void {
  useEffect(() => {
    if (!(import.meta.env.DEV || __WEB_DEV_MODE__)) return;
    if (typeof document === 'undefined') return;
    warnUnnamedButtons(document);

    const observer = new MutationObserver((mutations) => {
      for (const mutation of mutations) {
        for (const node of mutation.addedNodes) {
          if (node instanceof HTMLElement) {
            warnUnnamedButtons(node);
          }
        }
      }
    });

    observer.observe(document.body, { childList: true, subtree: true });
    return () => observer.disconnect();
  }, []);
}
