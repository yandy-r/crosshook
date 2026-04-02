import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import './styles/theme.css';
import './styles/focus.css';
import './styles/layout.css';
import './styles/sidebar.css';
import './styles/console-drawer.css';
import './styles/themed-select.css';
import './styles/collapsible-section.css';
import './styles/library.css';

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
