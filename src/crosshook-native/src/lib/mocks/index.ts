import { registerSettings } from './handlers/settings';
import { registerProfile } from './handlers/profile';

export type Handler = (args: unknown) => unknown | Promise<unknown>;

export function registerMocks(): Map<string, Handler> {
  const map = new Map<string, Handler>();
  registerSettings(map);
  registerProfile(map);
  return map;
}
