import { useMemo } from 'react';
import { AppShell } from '@/components/layout/AppShell';
import { CollectionsProvider } from '@/context/CollectionsContext';
import { HostReadinessProvider } from '@/context/HostReadinessContext';
import { ProfileProvider } from '@/context/ProfileContext';
import { ProfileHealthProvider } from '@/context/ProfileHealthContext';
import { useAriaLabelHydration } from '@/hooks/useAccessibilityEnhancements';
import { useGamepadNav } from '@/hooks/useGamepadNav';
import { useScrollEnhance } from '@/hooks/useScrollEnhance';
import { DevModeBanner } from '@/lib/DevModeBanner';
import { getActiveFixture } from '@/lib/fixture';
import { getActiveToggles, togglesToChipFragments } from '@/lib/toggles';

function handleGamepadBack(): void {
  const closeButtons = document.querySelectorAll<HTMLButtonElement>(
    '[data-crosshook-focus-root="modal"] [data-crosshook-modal-close]'
  );
  const closeButton = closeButtons[closeButtons.length - 1];
  closeButton?.click();
}

export function App() {
  const gamepadOptions = useMemo(() => ({ onBack: handleGamepadBack }), []);
  const gamepadNav = useGamepadNav(gamepadOptions);
  useScrollEnhance();
  useAriaLabelHydration();

  return (
    <main
      ref={gamepadNav.rootRef}
      className={`crosshook-app crosshook-focus-scope${__WEB_DEV_MODE__ ? ' crosshook-app--webdev' : ''}`}
    >
      {__WEB_DEV_MODE__ && (
        <DevModeBanner fixture={getActiveFixture()} toggles={togglesToChipFragments(getActiveToggles())} />
      )}
      <ProfileProvider>
        <ProfileHealthProvider>
          <HostReadinessProvider>
            <CollectionsProvider>
              <AppShell controllerMode={gamepadNav.controllerMode} />
            </CollectionsProvider>
          </HostReadinessProvider>
        </ProfileHealthProvider>
      </ProfileProvider>
    </main>
  );
}

export default App;
