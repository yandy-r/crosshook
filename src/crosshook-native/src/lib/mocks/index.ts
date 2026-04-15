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

import { registerCollections } from './handlers/collections';
import { registerCommunity } from './handlers/community';
import { registerHealth } from './handlers/health';
import { registerInstall } from './handlers/install';
import { registerLauncher } from './handlers/launcher';
import { registerLibrary } from './handlers/library';
import { registerOnboarding } from './handlers/onboarding';
import { registerProton } from './handlers/proton';
import { registerProtonDb } from './handlers/protondb';
import { registerProtonUp } from './handlers/protonup';
import { registerSystem } from './handlers/system';
import type { Handler } from './handlers/types';
import { registerUmuDatabase } from './handlers/umu_database';
import { registerUpdate } from './handlers/update';
import { wrapAllHandlers } from './wrapHandler';

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
