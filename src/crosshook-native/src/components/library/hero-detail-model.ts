/**
 * Pure types and helpers for in-shell Hero Detail mode (no hooks).
 */

export type HeroDetailTabId = 'overview' | 'profiles' | 'launch-options' | 'trainer' | 'history' | 'compatibility';

export const HERO_DETAIL_TABS: readonly { id: HeroDetailTabId; label: string }[] = [
  { id: 'overview', label: 'Overview' },
  { id: 'profiles', label: 'Profiles' },
  { id: 'launch-options', label: 'Launch options' },
  { id: 'trainer', label: 'Trainer' },
  { id: 'history', label: 'History' },
  { id: 'compatibility', label: 'Compatibility' },
] as const;

/**
 * Stable `data-testid` values for Hero Detail tabs (Phase 1 scope: profiles, launch-options).
 * Note: tab id `launch-options` maps to the shortened testid `hero-detail-launch-tab` (not `hero-detail-launch-options-tab`).
 */
export const HERO_DETAIL_TAB_TESTIDS: Partial<Record<HeroDetailTabId, string>> = {
  profiles: 'hero-detail-profiles-tab',
  // tab id `launch-options` → shortened testid `hero-detail-launch-tab`
  'launch-options': 'hero-detail-launch-tab',
};

/** Returns the stable testid for a given tab, or `undefined` if none is defined for that tab. */
export function heroDetailTabTestId(tabId: HeroDetailTabId): string | undefined {
  return HERO_DETAIL_TAB_TESTIDS[tabId];
}

export function displayPath(value: string | null | undefined): string {
  const trimmed = value?.trim();
  return trimmed && trimmed.length > 0 ? trimmed : 'Not set';
}

/**
 * Hero precedence: custom background → SteamGridDB background → SteamGridDB hero → Steam header_image → none.
 */
export function resolveGameDetailsHero(args: {
  customBgPath?: string;
  bg: { url: string | null; loading: boolean };
  hero: { url: string | null; loading: boolean };
  headerImage: string | null;
  metaLoading: boolean;
}): { url: string | null; showSkeleton: boolean } {
  const custom = args.customBgPath?.trim();
  if (custom && (args.bg.loading || args.bg.url)) {
    return { url: args.bg.url, showSkeleton: args.bg.loading };
  }
  if (args.bg.loading) {
    return { url: null, showSkeleton: true };
  }
  if (args.bg.url) {
    return { url: args.bg.url, showSkeleton: false };
  }
  if (args.hero.loading) {
    return { url: null, showSkeleton: true };
  }
  if (args.hero.url) {
    return { url: args.hero.url, showSkeleton: false };
  }
  if (args.metaLoading) {
    return { url: null, showSkeleton: true };
  }
  if (args.headerImage) {
    return { url: args.headerImage, showSkeleton: false };
  }
  return { url: null, showSkeleton: false };
}
