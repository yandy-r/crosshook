import type { ComponentType, SVGProps } from 'react';
import GameInspector from '@/components/library/GameInspector';
import type { LibraryCardData } from '@/types/library';
import {
  CommunityArt,
  CompatibilityArt,
  DiscoverArt,
  HealthDashboardArt,
  HostToolsArt,
  InstallArt,
  LaunchArt,
  LibraryArt,
  ProfilesArt,
  ProtonManagerArt,
  SettingsArt,
} from './PageBanner';
import type { AppRoute } from './Sidebar';

/** Selection payload for route-level inspector bodies (library uses `LibraryCardData`). */
export type SelectedGame = LibraryCardData;

/** Props passed into optional per-route inspector bodies (library wires actions from `LibraryPage`). */
export type InspectorBodyProps = {
  selection?: SelectedGame;
  onLaunch?: (name: string) => void;
  onEditProfile?: (name: string) => void;
  /** `current` is the profile's `isFavorite` before the toggle (matches `LibraryPage`). */
  onToggleFavorite?: (name: string, current: boolean) => void;
};

export interface RouteMetadataEntry {
  /** Label shown in the sidebar and status row — must stay in sync with navigation. */
  navLabel: string;
  /** Uppercase-style section label for the route banner eyebrow. */
  sectionEyebrow: string;
  bannerTitle: string;
  bannerSummary: string;
  Art: ComponentType<SVGProps<SVGSVGElement>>;
  /** Optional right-rail inspector body for this route. */
  inspectorComponent?: ComponentType<InspectorBodyProps>;
}

export const ROUTE_METADATA: Record<AppRoute, RouteMetadataEntry> = {
  library: {
    navLabel: 'Library',
    sectionEyebrow: 'Game',
    bannerTitle: 'Library',
    bannerSummary: 'Browse your saved profiles, favorites, and launch shortcuts in one place.',
    Art: LibraryArt,
    inspectorComponent: GameInspector,
  },
  profiles: {
    navLabel: 'Profiles',
    sectionEyebrow: 'Game',
    bannerTitle: 'Profiles',
    bannerSummary: 'Create, select, and maintain profiles for each game and trainer setup.',
    Art: ProfilesArt,
  },
  launch: {
    navLabel: 'Launch',
    sectionEyebrow: 'Game',
    bannerTitle: 'Launch',
    bannerSummary: 'Run the game or trainer with the active profile’s launch configuration.',
    Art: LaunchArt,
  },
  install: {
    navLabel: 'Install & Run',
    sectionEyebrow: 'Setup',
    bannerTitle: 'Install & Run',
    bannerSummary:
      'Install games, apply updates, or run an arbitrary Windows EXE or MSI without committing it to a profile.',
    Art: InstallArt,
  },
  community: {
    navLabel: 'Browse',
    sectionEyebrow: 'Community',
    bannerTitle: 'Browse',
    bannerSummary: 'Search shared profiles from your taps and import them into your library.',
    Art: CommunityArt,
  },
  discover: {
    navLabel: 'Discover',
    sectionEyebrow: 'Community',
    bannerTitle: 'Discover',
    bannerSummary: 'Search community trainer sources linked from CrossHook (external sites only).',
    Art: DiscoverArt,
  },
  compatibility: {
    navLabel: 'Compatibility',
    sectionEyebrow: 'Community',
    bannerTitle: 'Compatibility',
    bannerSummary: 'Inspect trainer compatibility data and manage Proton installs.',
    Art: CompatibilityArt,
  },
  settings: {
    navLabel: 'Settings',
    sectionEyebrow: 'App',
    bannerTitle: 'Settings',
    bannerSummary: 'Startup behavior, storage paths, integrations, and recent file history.',
    Art: SettingsArt,
  },
  health: {
    navLabel: 'Health',
    sectionEyebrow: 'Dashboards',
    bannerTitle: 'Health',
    bannerSummary: 'Validate profiles, review issues, and track launch health across your library.',
    Art: HealthDashboardArt,
  },
  'host-tools': {
    navLabel: 'Host Tools',
    sectionEyebrow: 'Dashboards',
    bannerTitle: 'Host Tools',
    bannerSummary:
      'Inspect detected host tools, capability state, and install guidance for everything CrossHook delegates to your host.',
    Art: HostToolsArt,
  },
  'proton-manager': {
    navLabel: 'Proton Manager',
    sectionEyebrow: 'Dashboards',
    bannerTitle: 'Proton Manager',
    bannerSummary: 'Download, install, and manage Proton compatibility tool versions for your Steam library.',
    Art: ProtonManagerArt,
  },
};

/** Sidebar + status row labels (single source of truth). */
export const ROUTE_NAV_LABEL: Record<AppRoute, string> = {
  library: ROUTE_METADATA.library.navLabel,
  profiles: ROUTE_METADATA.profiles.navLabel,
  launch: ROUTE_METADATA.launch.navLabel,
  install: ROUTE_METADATA.install.navLabel,
  community: ROUTE_METADATA.community.navLabel,
  discover: ROUTE_METADATA.discover.navLabel,
  compatibility: ROUTE_METADATA.compatibility.navLabel,
  settings: ROUTE_METADATA.settings.navLabel,
  health: ROUTE_METADATA.health.navLabel,
  'host-tools': ROUTE_METADATA['host-tools'].navLabel,
  'proton-manager': ROUTE_METADATA['proton-manager'].navLabel,
};
