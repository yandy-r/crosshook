import * as Tabs from '@radix-ui/react-tabs';
import type { ComponentType, SVGProps } from 'react';
import { CollectionsSidebar } from '../collections/CollectionsSidebar';
import {
  BrowseIcon,
  CompatibilityIcon,
  DiscoverIcon,
  HealthIcon,
  HostToolsIcon,
  InstallIcon,
  LaunchIcon,
  LibraryIcon,
  ProfilesIcon,
  ProtonManagerIcon,
  SettingsIcon,
} from '../icons/SidebarIcons';
import { ROUTE_NAV_LABEL } from './routeMetadata';

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
  onNavigate: (route: AppRoute) => void;
  controllerMode: boolean;
  lastProfile: string;
  onOpenCollection: (id: string) => void;
}

interface SidebarSectionItem {
  route: AppRoute;
  label: string;
  icon: ComponentType<SVGProps<SVGSVGElement>>;
}

interface SidebarSection {
  label: string;
  items: SidebarSectionItem[];
}

const SIDEBAR_SECTIONS: SidebarSection[] = [
  {
    label: 'Game',
    items: [
      { route: 'library', label: ROUTE_NAV_LABEL.library, icon: LibraryIcon },
      { route: 'profiles', label: ROUTE_NAV_LABEL.profiles, icon: ProfilesIcon },
      { route: 'launch', label: ROUTE_NAV_LABEL.launch, icon: LaunchIcon },
    ],
  },
  {
    label: 'Setup',
    items: [{ route: 'install', label: ROUTE_NAV_LABEL.install, icon: InstallIcon }],
  },
  {
    label: 'Dashboards',
    items: [
      { route: 'health', label: ROUTE_NAV_LABEL.health, icon: HealthIcon },
      { route: 'host-tools', label: ROUTE_NAV_LABEL['host-tools'], icon: HostToolsIcon },
      { route: 'proton-manager', label: ROUTE_NAV_LABEL['proton-manager'], icon: ProtonManagerIcon },
    ],
  },
  {
    label: 'Community',
    items: [
      { route: 'community', label: ROUTE_NAV_LABEL.community, icon: BrowseIcon },
      { route: 'discover', label: ROUTE_NAV_LABEL.discover, icon: DiscoverIcon },
      { route: 'compatibility', label: ROUTE_NAV_LABEL.compatibility, icon: CompatibilityIcon },
    ],
  },
];

function SidebarTrigger({
  activeRoute,
  onNavigate,
  route,
  label,
  icon: Icon,
}: SidebarSectionItem & Pick<SidebarProps, 'activeRoute' | 'onNavigate'>) {
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

function StatusRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="crosshook-sidebar__status">
      <span className="crosshook-sidebar__status-label">{label}</span>
      <span>{value}</span>
    </div>
  );
}

export function Sidebar({ activeRoute, onNavigate, controllerMode, lastProfile, onOpenCollection }: SidebarProps) {
  const controllerLabel = controllerMode ? 'On' : 'Off';
  const profileLabel = lastProfile.trim() || 'No profile selected';

  return (
    <aside className="crosshook-sidebar" data-crosshook-focus-zone="sidebar" aria-label="CrossHook navigation">
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

      <Tabs.List className="crosshook-sidebar__nav" aria-label="CrossHook sections">
        {SIDEBAR_SECTIONS[0] ? (
          <div className="crosshook-sidebar__section" key={SIDEBAR_SECTIONS[0].label}>
            <div className="crosshook-sidebar__section-label">{SIDEBAR_SECTIONS[0].label}</div>
            <div className="crosshook-sidebar__section-items">
              {SIDEBAR_SECTIONS[0].items.map((item) => (
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
        ) : null}

        <CollectionsSidebar onOpenCollection={onOpenCollection} />

        {SIDEBAR_SECTIONS.slice(1).map((section) => (
          <div className="crosshook-sidebar__section" key={section.label}>
            <div className="crosshook-sidebar__section-label">{section.label}</div>
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
