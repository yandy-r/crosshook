import type { ComponentType, SVGProps } from 'react';

import type { AppRoute } from './Sidebar';
import {
  CommunityArt,
  CompatibilityArt,
  DiscoverArt,
  HealthDashboardArt,
  InstallArt,
  LaunchArt,
  LibraryArt,
  ProfilesArt,
  SettingsArt,
} from './PageBanner';

export interface RouteMetadataEntry {
  /** Label shown in the sidebar and status row — must stay in sync with navigation. */
  navLabel: string;
  /** Uppercase-style section label for the route banner eyebrow. */
  sectionEyebrow: string;
  bannerTitle: string;
  bannerSummary: string;
  Art: ComponentType<SVGProps<SVGSVGElement>>;
}

export const ROUTE_METADATA: Record<AppRoute, RouteMetadataEntry> = {
  library: {
    navLabel: 'Library',
    sectionEyebrow: 'Game',
    bannerTitle: 'Library',
    bannerSummary: 'Browse your saved profiles, favorites, and launch shortcuts in one place.',
    Art: LibraryArt,
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
    navLabel: 'Install Game',
    sectionEyebrow: 'Setup',
    bannerTitle: 'Install Game',
    bannerSummary: 'Install or update games, then review and save generated profiles.',
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
};
