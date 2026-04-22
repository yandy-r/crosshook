import type { AppRoute } from '@/components/layout/Sidebar';

const VALID_APP_ROUTES: Record<AppRoute, true> = {
  library: true,
  profiles: true,
  launch: true,
  install: true,
  community: true,
  discover: true,
  compatibility: true,
  settings: true,
  health: true,
  'host-tools': true,
  'proton-manager': true,
};

export function isAppRoute(value: string): value is AppRoute {
  // biome-ignore lint: Object.hasOwn needs ES2022 lib; keep own-property check for route keys only
  return Object.prototype.hasOwnProperty.call(VALID_APP_ROUTES, value);
}
