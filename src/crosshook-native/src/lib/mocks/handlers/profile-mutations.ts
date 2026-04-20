// Profile mutation handlers: save, save variants, delete, duplicate, rename, favorites, import/export.
// See `lib/mocks/README.md`.
// All error messages MUST start with `[dev-mock]` to participate in the
// `.github/workflows/release.yml` "Verify no mock code in production bundle"
// sentinel.

import type { DuplicateProfileResult, GameProfile, GamescopeConfig, MangoHudConfig } from '../../../types/profile';
import { createDefaultProfile } from '../../../types/profile';
import { emitMockEvent } from '../eventBus';
import { getStore } from '../store';
import { profileFavorites } from './profile-core';
import { appendRevision } from './profile-history';
import { withProfileFixtureGate } from './profile-utils';
import type { Handler } from './types';

export function registerProfileMutations(map: Map<string, Handler>): void {
  map.set(
    'profile_save',
    withProfileFixtureGate('profile_save', async (args) => {
      const { name, data } = args as { name: string; data: GameProfile };
      const trimmed = name.trim();
      if (!trimmed) {
        throw new Error('[dev-mock] profile_save: name is required');
      }
      const store = getStore();
      store.profiles.set(trimmed, structuredClone(data));
      appendRevision(trimmed, 'manual_save');
      emitMockEvent('profiles-changed', { name: trimmed, action: 'save' });
      return null;
    })
  );

  map.set(
    'profile_save_launch_optimizations',
    withProfileFixtureGate('profile_save_launch_optimizations', async (args) => {
      const { name, optimizations } = args as {
        name: string;
        optimizations: { enabled_option_ids: string[]; switch_active_preset?: string };
      };
      const trimmed = name.trim();
      const store = getStore();
      const existing = store.profiles.get(trimmed);
      if (!existing) {
        throw new Error(`[dev-mock] profile_save_launch_optimizations: profile not found: ${trimmed}`);
      }
      const updated: GameProfile = {
        ...existing,
        launch: {
          ...existing.launch,
          optimizations: { enabled_option_ids: [...optimizations.enabled_option_ids] },
          ...(optimizations.switch_active_preset !== undefined
            ? { active_preset: optimizations.switch_active_preset }
            : {}),
        },
      };
      store.profiles.set(trimmed, updated);
      appendRevision(trimmed, 'launch_optimization_save');
      emitMockEvent('profiles-changed', { name: trimmed, action: 'save-launch-optimizations' });
      return null;
    })
  );

  map.set(
    'profile_save_gamescope_config',
    withProfileFixtureGate('profile_save_gamescope_config', async (args) => {
      const { name, config } = args as { name: string; config: GamescopeConfig };
      const trimmed = name.trim();
      const store = getStore();
      const existing = store.profiles.get(trimmed);
      if (!existing) {
        throw new Error(`[dev-mock] profile_save_gamescope_config: profile not found: ${trimmed}`);
      }
      store.profiles.set(trimmed, {
        ...existing,
        launch: { ...existing.launch, gamescope: structuredClone(config) },
      });
      appendRevision(trimmed, 'manual_save');
      emitMockEvent('profiles-changed', { name: trimmed, action: 'save-gamescope-config' });
      return null;
    })
  );

  map.set(
    'profile_save_trainer_gamescope_config',
    withProfileFixtureGate('profile_save_trainer_gamescope_config', async (args) => {
      const { name, config } = args as { name: string; config: GamescopeConfig };
      const trimmed = name.trim();
      const store = getStore();
      const existing = store.profiles.get(trimmed);
      if (!existing) {
        throw new Error(`[dev-mock] profile_save_trainer_gamescope_config: profile not found: ${trimmed}`);
      }
      store.profiles.set(trimmed, {
        ...existing,
        launch: { ...existing.launch, trainer_gamescope: structuredClone(config) },
      });
      appendRevision(trimmed, 'manual_save');
      emitMockEvent('profiles-changed', { name: trimmed, action: 'save-trainer-gamescope-config' });
      return null;
    })
  );

  map.set(
    'profile_save_mangohud_config',
    withProfileFixtureGate('profile_save_mangohud_config', async (args) => {
      const { name, config } = args as { name: string; config: MangoHudConfig };
      const trimmed = name.trim();
      const store = getStore();
      const existing = store.profiles.get(trimmed);
      if (!existing) {
        throw new Error(`[dev-mock] profile_save_mangohud_config: profile not found: ${trimmed}`);
      }
      store.profiles.set(trimmed, {
        ...existing,
        launch: { ...existing.launch, mangohud: structuredClone(config) },
      });
      appendRevision(trimmed, 'manual_save');
      emitMockEvent('profiles-changed', { name: trimmed, action: 'save-mangohud-config' });
      return null;
    })
  );

  map.set(
    'profile_delete',
    withProfileFixtureGate('profile_delete', async (args) => {
      const { name } = args as { name: string };
      const trimmed = name.trim();
      const store = getStore();
      if (!store.profiles.has(trimmed)) {
        throw new Error(`[dev-mock] profile_delete: profile not found: ${trimmed}`);
      }
      store.profiles.delete(trimmed);
      profileFavorites.delete(trimmed);
      // Import from profile-history to clean up history
      const { profileConfigHistory } = await import('./profile-history');
      profileConfigHistory.delete(trimmed);
      if (store.activeProfileId === trimmed) {
        store.activeProfileId = store.profiles.size > 0 ? (store.profiles.keys().next().value ?? null) : null;
      }
      emitMockEvent('profiles-changed', { name: trimmed, action: 'delete' });
      return null;
    })
  );

  map.set(
    'profile_duplicate',
    withProfileFixtureGate('profile_duplicate', async (args) => {
      const { name } = args as { name: string };
      const trimmed = name.trim();
      const store = getStore();
      const source = store.profiles.get(trimmed);
      if (!source) {
        throw new Error(`[dev-mock] profile_duplicate: profile not found: ${trimmed}`);
      }

      // Generate a unique copy name (mirrors Rust copy-name logic)
      let copyName = `${trimmed} (Copy)`;
      let counter = 2;
      while (store.profiles.has(copyName)) {
        copyName = `${trimmed} (Copy ${counter})`;
        counter++;
      }

      const copy = structuredClone(source);
      copy.game = { ...copy.game, name: copyName };
      store.profiles.set(copyName, copy);
      appendRevision(copyName, 'manual_save');

      const result: DuplicateProfileResult = {
        name: copyName,
        profile: structuredClone(copy),
      };
      emitMockEvent('profiles-changed', { name: copyName, action: 'duplicate' });
      return result;
    })
  );

  map.set(
    'profile_rename',
    withProfileFixtureGate('profile_rename', async (args) => {
      const { oldName, newName } = args as { oldName: string; newName: string };
      const trimmedOld = oldName.trim();
      const trimmedNew = newName.trim();
      if (!trimmedNew) {
        throw new Error('[dev-mock] profile_rename: new name must not be empty');
      }
      const store = getStore();
      const existing = store.profiles.get(trimmedOld);
      if (!existing) {
        throw new Error(`[dev-mock] profile_rename: profile not found: ${trimmedOld}`);
      }
      if (store.profiles.has(trimmedNew)) {
        throw new Error(`[dev-mock] profile_rename: a profile named "${trimmedNew}" already exists`);
      }

      const renamed = structuredClone(existing);
      renamed.game = { ...renamed.game, name: trimmedNew };
      store.profiles.set(trimmedNew, renamed);
      store.profiles.delete(trimmedOld);

      if (profileFavorites.has(trimmedOld)) {
        profileFavorites.delete(trimmedOld);
        profileFavorites.add(trimmedNew);
      }

      // Import from profile-history to handle history migration
      const { profileConfigHistory } = await import('./profile-history');
      const historyRows = profileConfigHistory.get(trimmedOld);
      if (historyRows !== undefined) {
        profileConfigHistory.set(trimmedNew, historyRows);
        profileConfigHistory.delete(trimmedOld);
      }

      if (store.activeProfileId === trimmedOld) {
        store.activeProfileId = trimmedNew;
      }

      emitMockEvent('profiles-changed', { name: trimmedNew, action: 'rename' });
      // Returns had_launcher boolean (always false in mock — no real launcher files)
      return false;
    })
  );

  map.set(
    'profile_set_favorite',
    withProfileFixtureGate('profile_set_favorite', async (args) => {
      const { name, favorite } = args as { name: string; favorite: boolean };
      const trimmed = name.trim();
      if (favorite) {
        profileFavorites.add(trimmed);
      } else {
        profileFavorites.delete(trimmed);
      }
      emitMockEvent('profiles-changed', { name: trimmed, action: 'favorite' });
      return null;
    })
  );

  map.set(
    'profile_import_legacy',
    withProfileFixtureGate('profile_import_legacy', async (args) => {
      const { path } = args as { path: string };
      // Derive a name from the file stem
      const segments = path.replace(/\\/g, '/').split('/');
      const filename = segments[segments.length - 1] ?? 'imported';
      const stem = filename.replace(/\.[^.]+$/, '').trim() || 'imported';
      const store = getStore();
      const base = createDefaultProfile();
      const imported: GameProfile = {
        ...base,
        game: {
          ...base.game,
          name: stem,
          executable_path: `/mock/games/${stem}.exe`,
        },
      };
      store.profiles.set(stem, imported);
      appendRevision(stem, 'import');
      emitMockEvent('profiles-changed', { name: stem, action: 'import' });
      return structuredClone(imported);
    })
  );

  map.set(
    'profile_export_toml',
    withProfileFixtureGate('profile_export_toml', async (args) => {
      const { name, data } = args as { name: string; data: GameProfile };
      // Return a minimal TOML-like stub for the frontend to display
      return `# CrossHook Profile Export\n# Profile: ${name}\n[game]\nname = "${data.game.name}"\nexecutable_path = "${data.game.executable_path}"\n`;
    })
  );
}
