// Profile mock handler orchestration. See `lib/mocks/README.md`.
// All error messages MUST start with `[dev-mock]` to participate in the
// `.github/workflows/release.yml` "Verify no mock code in production bundle"
// sentinel.
//
// This module delegates to smaller specialized modules:
// - profile-core: list, summaries, favorites, load (shell-critical handlers)
// - profile-mutations: save, delete, duplicate, rename, favorites, import/export
// - profile-presets: bundled and manual optimization preset handlers
// - profile-history: config history, diff, rollback, mark known good
// - profile-utils: shared fixture helpers and collection defaults merge

import { profileFavorites, registerProfileCore } from './profile-core';
import { registerProfileHistory, resetProfileHistoryState } from './profile-history';
import { registerProfileMutations } from './profile-mutations';
import { registerProfilePresets } from './profile-presets';
import type { Handler } from './types';

export function registerProfile(map: Map<string, Handler>): void {
  registerProfileCore(map);
  registerProfileMutations(map);
  registerProfilePresets(map);
  registerProfileHistory(map);
}

export function resetProfileMockState(): void {
  profileFavorites.clear();
  resetProfileHistoryState();
}
