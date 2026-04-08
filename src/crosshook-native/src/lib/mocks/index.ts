import { registerSettings } from './handlers/settings';
import { registerProfile } from './handlers/profile';
import { registerLaunch } from './handlers/launch';
import { registerInstall } from './handlers/install';
import { registerUpdate } from './handlers/update';
import { registerHealth } from './handlers/health';
import { registerOnboarding } from './handlers/onboarding';
import { registerProton } from './handlers/proton';
import { registerProtonUp } from './handlers/protonup';
import { registerProtonDb } from './handlers/protondb';
import { registerCommunity } from './handlers/community';
import { registerLauncher } from './handlers/launcher';
import { registerLibrary } from './handlers/library';
import { registerSystem } from './handlers/system';

export type Handler = (args: unknown) => unknown | Promise<unknown>;

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

  return map;
}
