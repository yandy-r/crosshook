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
  return value in VALID_APP_ROUTES;
}
