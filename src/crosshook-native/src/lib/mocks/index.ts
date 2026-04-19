import { registerLaunch } from './handlers/launch';
import { registerProfile } from './handlers/profile';
import { registerSettings } from './handlers/settings';

export type { FixtureState } from '../fixture';
// Re-export the fixture switcher and orthogonal debug toggles. Handlers import
// `Handler` from `./handlers/types` and `getActiveFixture` from `../../fixture` to
// avoid a circular dependency with this barrel. Logic lives in `../fixture` and
// `../toggles` so they stay statically importable from production code without
// dragging this dev-only module into the bundle.
export { getActiveFixture } from '../fixture';
export type { DebugToggles } from '../toggles';
export { getActiveToggles, togglesToChipFragments } from '../toggles';

import { resetBrowserEventBus } from '../events';

import { registerCollections, resetCollectionsMockState } from './handlers/collections';
import { registerCommunity, resetCommunityMockState } from './handlers/community';
import { registerHealth, resetHealthMockState } from './handlers/health';
import { registerInstall, resetInstallMockState } from './handlers/install';
import { resetLaunchMockState } from './handlers/launch';
import { registerLauncher } from './handlers/launcher';
import { registerLibrary } from './handlers/library';
import { registerOnboarding, resetOnboardingMockState } from './handlers/onboarding';
import { resetProfileMockState } from './handlers/profile';
import { registerProton } from './handlers/proton';
import { registerProtonDb, resetProtonDbMockState } from './handlers/protondb';
import { registerProtonUp, resetProtonUpMockState } from './handlers/protonup';
import { registerSystem, resetSystemMockState } from './handlers/system';
import type { Handler } from './handlers/types';
import { registerUmuDatabase } from './handlers/umu_database';
import { registerUpdate, resetUpdateMockState } from './handlers/update';
import { resetStore } from './store';
import { resetWrappedHandlerState, wrapAllHandlers } from './wrapHandler';

export type { Handler };

export function registerMocks(): Map<string, Handler> {
  const map = new Map<string, Handler>();

  // Boot-critical (Phase 1)
  registerSettings(map);
  registerProfile(map);

  // Phase 2 domain handlers
  registerLaunch(map);
  registerInstall(map);
  registerUpdate(map);
  registerHealth(map);
  registerOnboarding(map);
  registerProton(map);
  registerProtonUp(map);
  registerProtonDb(map);
  registerCommunity(map);
  registerLauncher(map);
  registerLibrary(map);
  registerSystem(map);
  registerCollections(map);
  registerUmuDatabase(map);

  // Wrap every handler with the orthogonal debug-toggle middleware
  // (`?delay=`, `?errors=true`). MUST run AFTER every register*() call so
  // every entry in the map is wrapped exactly once. See `wrapHandler.ts` for
  // the BR-11 shell-critical read exemption rules.
  return wrapAllHandlers(map);
}

export function resetMockEnvironment(): void {
  resetBrowserEventBus();
  resetStore();
  resetCollectionsMockState();
  resetCommunityMockState();
  resetHealthMockState();
  resetInstallMockState();
  resetLaunchMockState();
  resetOnboardingMockState();
  resetProfileMockState();
  resetProtonDbMockState();
  resetProtonUpMockState();
  resetSystemMockState();
  resetUpdateMockState();
  resetWrappedHandlerState();
}
