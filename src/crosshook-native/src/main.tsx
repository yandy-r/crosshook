import React from 'react';
import ReactDOM from 'react-dom/client';
import '@/lib/plugin-stubs/convertFileSrc';
import {
  FORCED_COLORS_QUERY,
  HIGH_CONTRAST_THEME,
  PREFERS_CONTRAST_QUERY,
  resolveHighContrast,
  resolveMotion,
  THEME_ATTRIBUTE,
} from '@/hooks/useAccessibilityPreferences';
import { MOTION_ATTRIBUTE, REDUCED_MOTION_QUERY } from '@/lib/motion';
import App from './App';
import './styles/theme.css';
import './styles/utilities.css';
import './styles/focus.css';
import './styles/layout.css';
import './styles/dashboard-routes.css';
import './styles/editor-routes.css';
import './styles/install-routes.css';
import './styles/settings-routes.css';
import './styles/community-routes.css';
import './styles/discover-routes.css';
import './styles/onboarding-wizard.css';
import './styles/sidebar.css';
import './styles/console-drawer.css';
import './styles/themed-select.css';
import './styles/collapsible-section.css';
import './styles/library.css';
import './styles/palette.css';
import './styles/hero-detail.css';
import './styles/breadcrumb.css';
import './styles/collections-sidebar.css';
import './styles/mods.css';

if (import.meta.env.DEV) {
  void import('./lib/ipc').then(({ callCommand }) => {
    window.__CROSSHOOK_DEV__ = { callCommand };
  });
}

// Pre-render bootstrap from OS preferences ('auto' until settings load);
// AccessibilityPreferenceSync takes ownership once settings resolve.
const matchesQuery = (query: string): boolean => !!window.matchMedia?.(query).matches;
document.documentElement.setAttribute(MOTION_ATTRIBUTE, resolveMotion('auto', matchesQuery(REDUCED_MOTION_QUERY)));
if (resolveHighContrast('auto', matchesQuery(PREFERS_CONTRAST_QUERY), matchesQuery(FORCED_COLORS_QUERY))) {
  document.documentElement.setAttribute(THEME_ATTRIBUTE, HIGH_CONTRAST_THEME);
}

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
