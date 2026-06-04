// NOTE(hero-detail-consolidation): delete with Phase 10 route removal.

import type { AppNavigateOptions, GameDetailOrigin } from '@/types/navigation';
import type { BreadcrumbSegment } from './Breadcrumb';
import type { AppRoute } from './Sidebar';

export function buildGameDetailTrail(
  origin: GameDetailOrigin | null | undefined,
  onNavigate: ((route: AppRoute, options?: AppNavigateOptions) => void) | undefined,
  terminalLabel: 'Edit profile' | 'Launch'
): BreadcrumbSegment[] | undefined {
  if (!origin || !onNavigate) {
    return undefined;
  }
  return [
    { label: 'Library', onNavigate: () => onNavigate('library') },
    {
      label: origin.displayName,
      onNavigate: () => onNavigate('library', { openGameDetail: origin.profileName }),
    },
    { label: terminalLabel },
  ];
}
