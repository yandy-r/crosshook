import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import * as Tabs from '@radix-ui/react-tabs';
import { Group, Panel, Separator, type PanelImperativeHandle } from 'react-resizable-panels';
import { listen } from '@tauri-apps/api/event';

import ContentArea from './components/layout/ContentArea';
import ControllerPrompts from './components/layout/ControllerPrompts';
import ConsoleDrawer from './components/layout/ConsoleDrawer';
import Sidebar, { type AppRoute } from './components/layout/Sidebar';
import { OnboardingWizard } from './components/OnboardingWizard';
import type { OnboardingCheckPayload } from './types/onboarding';
import { LaunchStateProvider } from './context/LaunchStateContext';
import { PreferencesProvider } from './context/PreferencesContext';
import { ProfileProvider, useProfileContext } from './context/ProfileContext';
import { ProfileHealthProvider } from './context/ProfileHealthContext';
import { useGamepadNav } from './hooks/useGamepadNav';
import { useScrollEnhance } from './hooks/useScrollEnhance';

const VALID_APP_ROUTES: Record<AppRoute, true> = {
  profiles: true,
  launch: true,
  install: true,
  community: true,
  compatibility: true,
  settings: true,
  health: true,
};

function isAppRoute(value: string): value is AppRoute {
  return value in VALID_APP_ROUTES;
}

function handleGamepadBack(): void {
  const closeButtons = document.querySelectorAll<HTMLButtonElement>(
    '[data-crosshook-focus-root="modal"] [data-crosshook-modal-close]',
  );
  const closeButton = closeButtons[closeButtons.length - 1];
  closeButton?.click();
}

function AppShell({ controllerMode }: { controllerMode: boolean }) {
  const { profileName, selectedProfile } = useProfileContext();
  const [route, setRoute] = useState<AppRoute>('profiles');
  const lastProfile = profileName.trim() || selectedProfile;
  const consolePanelRef = useRef<PanelImperativeHandle>(null);
  const [showOnboarding, setShowOnboarding] = useState(false);

  useLayoutEffect(() => {
    consolePanelRef.current?.collapse();
  }, []);

  useEffect(() => {
    const p = listen<OnboardingCheckPayload>('onboarding-check', (event) => {
      if (event.payload.show && !event.payload.has_profiles) setShowOnboarding(true);
    });
    return () => { p.then(f => f()); };
  }, []);

  return (
    <PreferencesProvider activeProfileName={lastProfile}>
      <LaunchStateProvider>
      <Tabs.Root
        orientation="vertical"
        value={route}
        onValueChange={(value) => { if (isAppRoute(value)) setRoute(value); }}
      >
        <div className="crosshook-app-layout">
          <Group
            className="crosshook-shell-group"
            orientation="horizontal"
            resizeTargetMinimumSize={{ coarse: 36, fine: 12 }}
          >
            <Panel
              className="crosshook-shell-panel"
              defaultSize="20%"
              minSize="14%"
              maxSize="40%"
            >
              <Sidebar
                activeRoute={route}
                onNavigate={setRoute}
                controllerMode={controllerMode}
                lastProfile={lastProfile}
              />
            </Panel>
            <Separator className="crosshook-resize-handle crosshook-resize-handle--vertical" />
            <Panel className="crosshook-shell-panel" minSize="28%">
              <Group
                className="crosshook-shell-group"
                orientation="vertical"
                resizeTargetMinimumSize={{ coarse: 36, fine: 12 }}
              >
                <Panel
                  className="crosshook-shell-panel"
                  defaultSize="80%"
                  minSize="28%"
                >
                  <ContentArea route={route} onNavigate={setRoute} />
                </Panel>
                <Separator className="crosshook-resize-handle crosshook-resize-handle--horizontal" />
                <Panel
                  className="crosshook-shell-panel"
                  panelRef={consolePanelRef}
                  collapsible
                  collapsedSize="40px"
                  defaultSize="60%"
                  minSize="15%"
                  maxSize="75%"
                >
                  <ConsoleDrawer panelRef={consolePanelRef} />
                </Panel>
              </Group>
            </Panel>
          </Group>
        </div>
        {controllerMode ? <ControllerPrompts /> : null}
      </Tabs.Root>
      {showOnboarding && <OnboardingWizard open={showOnboarding} onComplete={() => setShowOnboarding(false)} onDismiss={() => setShowOnboarding(false)} />}
      </LaunchStateProvider>
    </PreferencesProvider>
  );
}

export function App() {
  const gamepadOptions = useMemo(() => ({ onBack: handleGamepadBack }), []);
  const gamepadNav = useGamepadNav(gamepadOptions);
  useScrollEnhance();

  return (
    <main ref={gamepadNav.rootRef} className="crosshook-app crosshook-focus-scope">
      <ProfileProvider>
        <ProfileHealthProvider>
          <AppShell controllerMode={gamepadNav.controllerMode} />
        </ProfileHealthProvider>
      </ProfileProvider>
    </main>
  );
}

export default App;
