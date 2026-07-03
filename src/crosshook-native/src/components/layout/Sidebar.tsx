import * as Tabs from '@radix-ui/react-tabs';
import type { ComponentType, SVGProps } from 'react';
import { useRovingTabindex } from '@/hooks/useRovingTabindex';
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
  /** Current library toolbar filter; drives `aria-pressed` on Favorites / Currently Playing. */
  activeLibraryFilter: LibraryFilterKey;
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
  items: SidebarRouteItem[];
}

type SidebarRouteTriggerProps = Omit<SidebarRouteItem, 'type'> & Pick<SidebarProps, 'activeRoute' | 'onNavigate'>;
type SidebarLibraryFilterTriggerProps = Omit<SidebarLibraryFilterItem, 'type' | 'badge'> &
  Pick<SidebarProps, 'onNavigate' | 'activeRoute' | 'activeLibraryFilter'> & {
    badge: string | number | undefined;
  };

// Rendered outside the Tabs.List: a tablist may only own tabs, so the
// collections group (plain buttons) lives in a sibling section.
const COLLECTIONS_SECTION_ITEMS: SidebarLibraryFilterItem[] = [
  { type: 'library-filter', filterKey: 'favorites', label: 'Favorites', icon: HeartIcon },
  { type: 'library-filter', filterKey: 'currentlyRunning', label: 'Currently Playing', icon: PlayIcon },
];

const SIDEBAR_SECTIONS: SidebarRouteSection[] = [
  {
    key: 'game',
    label: 'Game',
    items: [{ type: 'route', route: 'library', label: ROUTE_NAV_LABEL.library, icon: LibraryIcon }],
  },
  {
    key: 'setup',
    label: 'Setup',
    items: [{ type: 'route', route: 'install', label: ROUTE_NAV_LABEL.install, icon: InstallIcon }],
  },
  {
    key: 'dashboards',
    label: 'Dashboards',
    items: [
      { type: 'route', route: 'health', label: ROUTE_NAV_LABEL.health, icon: HealthIcon },
      { type: 'route', route: 'host-tools', label: ROUTE_NAV_LABEL['host-tools'], icon: HostToolsIcon },
      { type: 'route', route: 'proton-manager', label: ROUTE_NAV_LABEL['proton-manager'], icon: ProtonManagerIcon },
    ],
  },
  {
    key: 'community',
    label: 'Community',
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
  activeRoute,
  activeLibraryFilter,
  filterKey,
  label,
  icon: Icon,
  badge,
}: SidebarLibraryFilterTriggerProps) {
  const isPressed = activeRoute === 'library' && activeLibraryFilter === filterKey;

  return (
    <button
      type="button"
      className="crosshook-sidebar__item crosshook-collections-sidebar__item"
      data-roving-item=""
      tabIndex={-1}
      aria-pressed={isPressed}
      onClick={() => onNavigate('library', { libraryFilter: filterKey })}
      title={label}
    >
      <span className="crosshook-sidebar__item-icon" aria-hidden="true">
        <Icon />
      </span>
      <span className="crosshook-sidebar__item-label crosshook-collections-sidebar__item-name">{label}</span>
      {badge !== undefined ? <span className="crosshook-collections-sidebar__item-count">{badge}</span> : null}
    </button>
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

function SidebarRouteSectionBlock({
  section,
  activeRoute,
  onNavigate,
}: {
  section: SidebarRouteSection;
  activeRoute: AppRoute;
  onNavigate: (route: AppRoute, options?: AppNavigateOptions) => void;
}) {
  return (
    <div className="crosshook-sidebar__section">
      {/* Decorative inside the tablist — a heading is not a permitted tablist child. */}
      <div className="crosshook-sidebar__section-label" aria-hidden="true">
        {section.label}
      </div>
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
    </div>
  );
}

function SidebarCollectionsBlock({
  activeRoute,
  activeLibraryFilter,
  onNavigate,
  onOpenCollection,
  libraryFilterBadges,
}: {
  activeRoute: AppRoute;
  activeLibraryFilter: LibraryFilterKey;
  onNavigate: (route: AppRoute, options?: AppNavigateOptions) => void;
  onOpenCollection: (id: string) => void;
  libraryFilterBadges: Partial<Record<LibraryFilterKey, string | number>> | undefined;
}) {
  const rovingRef = useRovingTabindex({ itemSelector: '[data-roving-item]' });

  return (
    <div className="crosshook-sidebar__section">
      <h2 className="crosshook-sidebar__section-label">Collections</h2>
      <div
        ref={rovingRef}
        data-crosshook-roving="collections"
        role="group"
        aria-label="Library filters and collections"
      >
        <div className="crosshook-sidebar__section-items">
          {COLLECTIONS_SECTION_ITEMS.map((item) => (
            <SidebarLibraryFilterTrigger
              key={item.filterKey}
              onNavigate={onNavigate}
              activeRoute={activeRoute}
              activeLibraryFilter={activeLibraryFilter}
              filterKey={item.filterKey}
              label={item.label}
              icon={item.icon}
              badge={libraryFilterBadges?.[item.filterKey] ?? item.badge}
            />
          ))}
        </div>
        <CollectionsSidebar onOpenCollection={onOpenCollection} />
      </div>
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
  activeLibraryFilter,
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
          <SidebarRouteSectionBlock
            key={section.key}
            section={section}
            activeRoute={activeRoute}
            onNavigate={onNavigate}
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

      <SidebarCollectionsBlock
        activeRoute={activeRoute}
        activeLibraryFilter={activeLibraryFilter}
        onNavigate={onNavigate}
        onOpenCollection={onOpenCollection}
        libraryFilterBadges={libraryFilterBadges}
      />
    </aside>
  );
}

export default Sidebar;
