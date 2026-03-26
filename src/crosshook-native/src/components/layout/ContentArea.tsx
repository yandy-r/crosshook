import * as Tabs from '@radix-ui/react-tabs';

import CompatibilityPage from '../pages/CompatibilityPage';
import CommunityPage from '../pages/CommunityPage';
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
  switch (route) {
    case 'profiles':
      return (
        <Tabs.Content value="profiles" forceMount data-crosshook-focus-zone="content">
          <ProfilesPage />
        </Tabs.Content>
      );
    case 'launch':
      return (
        <Tabs.Content value="launch" forceMount data-crosshook-focus-zone="content">
          <LaunchPage />
        </Tabs.Content>
      );
    case 'install':
      return (
        <Tabs.Content value="install" forceMount data-crosshook-focus-zone="content">
          <InstallPage onNavigate={onNavigate} />
        </Tabs.Content>
      );
    case 'community':
      return (
        <Tabs.Content value="community" forceMount data-crosshook-focus-zone="content">
          <CommunityPage />
        </Tabs.Content>
      );
    case 'compatibility':
      return (
        <Tabs.Content value="compatibility" forceMount data-crosshook-focus-zone="content">
          <CompatibilityPage />
        </Tabs.Content>
      );
    case 'settings':
      return (
        <Tabs.Content value="settings" forceMount data-crosshook-focus-zone="content">
          <SettingsPage />
        </Tabs.Content>
      );
    default: {
      const _exhaustive: never = route;
      return _exhaustive;
    }
  }
}

export default ContentArea;
