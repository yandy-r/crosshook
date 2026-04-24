import * as Tabs from '@radix-ui/react-tabs';
import type { ComponentType, SVGProps } from 'react';
import type { LibraryFilterKey } from '@/types/library';
import type { AppNavigateOptions } from '@/types/navigation';
import { CollectionsSidebar } from '../collections/CollectionsSidebar';
import {
  BrowseIcon,
  CompatibilityIcon,
  DiscoverIcon,
  HealthIcon,
  HeartIcon,
  HostToolsIcon,
  InstallIcon,
  LibraryIcon,
  PlayIcon,
  ProtonManagerIcon,
  SettingsIcon,
} from '../icons/SidebarIcons';
import { ROUTE_NAV_LABEL } from './routeMetadata';
import { isSidebarCollapsedVariant, type SidebarVariant, sidebarWidthForVariant } from './sidebarVariants';

export type AppRoute =
  | 'library'
  | 'profiles'
  | 'launch'
  | 'install'
  | 'community'
  | 'discover'
  | 'compatibility'
  | 'settings'
  | 'health'
  | 'host-tools'
  | 'proton-manager';

export interface SidebarProps {
  activeRoute: AppRoute;
  onNavigate: (route: AppRoute, options?: AppNavigateOptions) => void;
  controllerMode: boolean;
  lastProfile: string;
  onOpenCollection: (id: string) => void;
  variant: SidebarVariant;
  libraryFilterBadges?: Partial<Record<LibraryFilterKey, string | number>>;
}

interface SidebarRouteItem {
  type: 'route';
  route: AppRoute;
  label: string;
  icon: ComponentType<SVGProps<SVGSVGElement>>;
}

interface SidebarLibraryFilterItem {
  type: 'library-filter';
  filterKey: LibraryFilterKey;
  label: string;
  icon: ComponentType<SVGProps<SVGSVGElement>>;
  badge?: string | number;
}

interface SidebarRouteSection {
  key: string;
  label: string;
  type: 'routes';
  items: SidebarRouteItem[];
}

interface SidebarCollectionsSection {
  key: 'collections';
  label: 'Collections';
  type: 'collections';
  items: SidebarLibraryFilterItem[];
}

type SidebarSection = SidebarRouteSection | SidebarCollectionsSection;
type SidebarRouteTriggerProps = Omit<SidebarRouteItem, 'type'> & Pick<SidebarProps, 'activeRoute' | 'onNavigate'>;
type SidebarLibraryFilterTriggerProps = Omit<SidebarLibraryFilterItem, 'type' | 'badge'> &
  Pick<SidebarProps, 'onNavigate'> & {
    badge: string | number | undefined;
  };

const SIDEBAR_SECTIONS: SidebarSection[] = [
  {
    key: 'game',
    label: 'Game',
    type: 'routes',
    items: [{ type: 'route', route: 'library', label: ROUTE_NAV_LABEL.library, icon: LibraryIcon }],
  },
  {
    key: 'collections',
    label: 'Collections',
    type: 'collections',
    items: [
      { type: 'library-filter', filterKey: 'favorites', label: 'Favorites', icon: HeartIcon },
      { type: 'library-filter', filterKey: 'currentlyRunning', label: 'Currently Playing', icon: PlayIcon },
    ],
  },
  {
    key: 'setup',
    label: 'Setup',
    type: 'routes',
    items: [{ type: 'route', route: 'install', label: ROUTE_NAV_LABEL.install, icon: InstallIcon }],
  },
  {
    key: 'dashboards',
    label: 'Dashboards',
    type: 'routes',
    items: [
      { type: 'route', route: 'health', label: ROUTE_NAV_LABEL.health, icon: HealthIcon },
      { type: 'route', route: 'host-tools', label: ROUTE_NAV_LABEL['host-tools'], icon: HostToolsIcon },
      { type: 'route', route: 'proton-manager', label: ROUTE_NAV_LABEL['proton-manager'], icon: ProtonManagerIcon },
    ],
  },
  {
    key: 'community',
    label: 'Community',
    type: 'routes',
    items: [
      { type: 'route', route: 'community', label: ROUTE_NAV_LABEL.community, icon: BrowseIcon },
      { type: 'route', route: 'discover', label: ROUTE_NAV_LABEL.discover, icon: DiscoverIcon },
      { type: 'route', route: 'compatibility', label: ROUTE_NAV_LABEL.compatibility, icon: CompatibilityIcon },
    ],
  },
];

function SidebarTrigger({ activeRoute, onNavigate, route, label, icon: Icon }: SidebarRouteTriggerProps) {
  const isCurrent = activeRoute === route;

  return (
    <Tabs.Trigger
      className="crosshook-sidebar__item"
      value={route}
      aria-current={isCurrent ? 'page' : undefined}
      onClick={() => onNavigate(route)}
      title={label}
    >
      <span className="crosshook-sidebar__item-icon" aria-hidden="true">
        <Icon />
      </span>
      <span className="crosshook-sidebar__item-label">{label}</span>
    </Tabs.Trigger>
  );
}

function SidebarLibraryFilterTrigger({
  onNavigate,
  filterKey,
  label,
  icon: Icon,
  badge,
}: SidebarLibraryFilterTriggerProps) {
  return (
    <Tabs.Trigger
      className="crosshook-sidebar__item crosshook-collections-sidebar__item"
      value={`library-filter-${filterKey}`}
      onClick={() => onNavigate('library', { libraryFilter: filterKey })}
      title={label}
    >
      <span className="crosshook-sidebar__item-icon" aria-hidden="true">
        <Icon />
      </span>
      <span className="crosshook-sidebar__item-label crosshook-collections-sidebar__item-name">{label}</span>
      {badge !== undefined ? <span className="crosshook-collections-sidebar__item-count">{badge}</span> : null}
    </Tabs.Trigger>
  );
}

function StatusRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="crosshook-sidebar__status">
      <span className="crosshook-sidebar__status-label">{label}</span>
      <span>{value}</span>
    </div>
  );
}

function SidebarSectionBlock({
  section,
  activeRoute,
  onNavigate,
  onOpenCollection,
  libraryFilterBadges,
}: {
  section: SidebarSection;
  activeRoute: AppRoute;
  onNavigate: (route: AppRoute, options?: AppNavigateOptions) => void;
  onOpenCollection: (id: string) => void;
  libraryFilterBadges: Partial<Record<LibraryFilterKey, string | number>> | undefined;
}) {
  return (
    <div className="crosshook-sidebar__section" key={section.key}>
      <h2 className="crosshook-sidebar__section-label">{section.label}</h2>
      {section.type === 'routes' ? (
        <div className="crosshook-sidebar__section-items">
          {section.items.map((item) => (
            <SidebarTrigger
              key={item.route}
              activeRoute={activeRoute}
              onNavigate={onNavigate}
              route={item.route}
              label={item.label}
              icon={item.icon}
            />
          ))}
        </div>
      ) : (
        <>
          <div className="crosshook-sidebar__section-items">
            {section.items.map((item) => (
              <SidebarLibraryFilterTrigger
                key={item.filterKey}
                onNavigate={onNavigate}
                filterKey={item.filterKey}
                label={item.label}
                icon={item.icon}
                badge={libraryFilterBadges?.[item.filterKey] ?? item.badge}
              />
            ))}
          </div>
          <CollectionsSidebar onOpenCollection={onOpenCollection} />
        </>
      )}
    </div>
  );
}

export function Sidebar({
  activeRoute,
  onNavigate,
  controllerMode,
  lastProfile,
  onOpenCollection,
  variant,
  libraryFilterBadges,
}: SidebarProps) {
  const controllerLabel = controllerMode ? 'On' : 'Off';
  const profileLabel = lastProfile.trim() || 'No profile selected';
  const collapsed = isSidebarCollapsedVariant(variant);
  const width = sidebarWidthForVariant(variant);

  return (
    <aside
      className="crosshook-sidebar"
      data-testid="sidebar"
      style={{ width: `${width}px` }}
      data-collapsed={collapsed ? 'true' : 'false'}
      data-crosshook-focus-zone="sidebar"
      data-sidebar-variant={variant}
      data-sidebar-width={width}
      aria-label="CrossHook navigation"
    >
      <div className="crosshook-sidebar__brand">
        <div className="crosshook-sidebar__brand-content">
          <p className="crosshook-sidebar__brand-title">CrossHook</p>
          <p className="crosshook-sidebar__brand-subtitle">Launch, install, and manage profiles</p>
        </div>
        <div className="crosshook-sidebar__brand-art" aria-hidden="true">
          <svg
            viewBox="0 0 64 64"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden="true"
          >
            {/* Crosshair outer ring */}
            <circle cx="32" cy="32" r="20" opacity={0.35} />
            <circle cx="32" cy="32" r="12" opacity={0.2} />
            {/* Crosshair lines */}
            <line x1="32" y1="8" x2="32" y2="18" opacity={0.3} />
            <line x1="32" y1="46" x2="32" y2="56" opacity={0.3} />
            <line x1="8" y1="32" x2="18" y2="32" opacity={0.3} />
            <line x1="46" y1="32" x2="56" y2="32" opacity={0.3} />
            {/* Hook shape in center */}
            <path d="M28 26 v10 a6 6 0 0 0 12 0" strokeWidth="2" opacity={0.5} />
            <line x1="28" y1="24" x2="28" y2="27" strokeWidth="2" opacity={0.5} />
            {/* Accent dot at center */}
            <circle cx="32" cy="32" r="2" fill="currentColor" opacity={0.25} stroke="none" />
          </svg>
        </div>
      </div>

      <Tabs.List className="crosshook-sidebar__nav crosshook-sidebar__nav--scroll" aria-label="CrossHook sections">
        {SIDEBAR_SECTIONS.map((section) => (
          <SidebarSectionBlock
            key={section.key}
            section={section}
            activeRoute={activeRoute}
            onNavigate={onNavigate}
            onOpenCollection={onOpenCollection}
            libraryFilterBadges={libraryFilterBadges}
          />
        ))}

        <div className="crosshook-sidebar__footer">
          <SidebarTrigger
            activeRoute={activeRoute}
            onNavigate={onNavigate}
            route="settings"
            label={ROUTE_NAV_LABEL.settings}
            icon={SettingsIcon}
          />

          <div className="crosshook-sidebar__status-group">
            <StatusRow label="Current view" value={ROUTE_NAV_LABEL[activeRoute]} />
            <StatusRow label="Controller" value={controllerLabel} />
            <StatusRow label="Last profile" value={profileLabel} />
          </div>
        </div>
      </Tabs.List>
    </aside>
  );
}

export default Sidebar;
