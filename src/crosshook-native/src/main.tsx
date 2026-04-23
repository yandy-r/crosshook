import React from 'react';
import ReactDOM from 'react-dom/client';
import '@/lib/plugin-stubs/convertFileSrc';
import App from './App';
import './styles/theme.css';
import './styles/utilities.css';
import './styles/focus.css';
import './styles/layout.css';
import './styles/dashboard-routes.css';
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
import './styles/collections-sidebar.css';

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
