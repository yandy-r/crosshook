import { useLayoutEffect, useRef } from 'react';
import * as Tabs from '@radix-ui/react-tabs';

import CompatibilityPage from '../pages/CompatibilityPage';
import CommunityPage from '../pages/CommunityPage';
import HealthDashboardPage from '../pages/HealthDashboardPage';
import InstallPage from '../pages/InstallPage';
import LaunchPage from '../pages/LaunchPage';
import ProfilesPage from '../pages/ProfilesPage';
import SettingsPage from '../pages/SettingsPage';
import type { AppRoute } from './Sidebar';

export interface ContentAreaProps {
  route: AppRoute;
  onNavigate?: (route: AppRoute) => void;
}

export function ContentArea({ route, onNavigate }: ContentAreaProps) {
  const scrollRef = useRef<HTMLDivElement>(null);

  useLayoutEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = 0;
      scrollRef.current.scrollLeft = 0;
    }
  }, [route]);

  const contentProps = {
    value: route,
    forceMount: true as const,
    'data-crosshook-focus-zone': 'content' as const,
  };

  function renderPage() {
    switch (route) {
      case 'profiles':
        return <ProfilesPage />;
      case 'launch':
        return <LaunchPage />;
      case 'install':
        return <InstallPage onNavigate={onNavigate} />;
      case 'community':
        return <CommunityPage />;
      case 'compatibility':
        return <CompatibilityPage />;
      case 'settings':
        return <SettingsPage />;
      case 'health':
        return <HealthDashboardPage onNavigate={onNavigate} />;
      default: {
        const _exhaustive: never = route;
        return _exhaustive;
      }
    }
  }

  return (
    <div ref={scrollRef} className="crosshook-content-area">
      <Tabs.Content key={route} {...contentProps}>
        {renderPage()}
      </Tabs.Content>
    </div>
  );
}

export default ContentArea;
