import type { Handler } from './types';
import { getActiveFixture } from '../../fixture';
import { getStore } from '../store';
import { emitMockEvent } from '../eventBus';
import type { ProfileSummary } from '../../../types/library';
import type {
  GameProfile,
  DuplicateProfileResult,
  BundledOptimizationPreset,
  GamescopeConfig,
  MangoHudConfig,
} from '../../../types/profile';
import { createDefaultProfile } from '../../../types/profile';
import type {
  ConfigRevisionSummary,
  ConfigDiffResult,
  ConfigRollbackResult,
} from '../../../types/profile-history';

// Module-scope state for features not tracked in MockStore
const profileFavorites = new Set<string>();

// Lightweight config revision history (profile name → ordered summaries, newest first)
const profileConfigHistory = new Map<string, ConfigRevisionSummary[]>();

let nextRevisionId = 1;

/** Same catalog as `profile_list_bundled_optimization_presets` — keep in sync for mock fidelity. */
function getBundledOptimizationPresets(): BundledOptimizationPreset[] {
  return [
    {
      preset_id: 'bundled/amd-fsr',
      display_name: 'AMD FSR Performance',
      vendor: 'amd',
      mode: 'performance',
      enabled_option_ids: ['use_gamemode', 'disable_vsync'],
      catalog_version: 1,
    },
    {
      preset_id: 'bundled/nvidia-dxvk',
      display_name: 'NVIDIA DXVK Quality',
      vendor: 'nvidia',
      mode: 'quality',
      enabled_option_ids: ['use_gamemode', 'enable_esync'],
      catalog_version: 1,
    },
  ];
}

function appendRevision(profileName: string, source: ConfigRevisionSummary['source']): ConfigRevisionSummary {
  const revision: ConfigRevisionSummary = {
    id: nextRevisionId++,
    profile_name_at_write: profileName,
    source,
    content_hash: `mock-hash-${nextRevisionId}`,
    source_revision_id: null,
    is_last_known_working: false,
    created_at: new Date().toISOString(),
  };
  const existing = profileConfigHistory.get(profileName) ?? [];
  profileConfigHistory.set(profileName, [revision, ...existing]);
  return revision;
}

// ---------------------------------------------------------------------------
// Fixture helpers (BR-11)
// ---------------------------------------------------------------------------
//
// `populated` (default) — current behavior, returns demo data
// `empty`               — list/load handlers return empty/null
// `error`               — fallible handlers throw; shell-critical reads still
//                          resolve so `<AppShell />` can mount
// `loading`             — non-shell-critical handlers never resolve
//                          (`new Promise(() => {})`); shell-critical reads
//                          still resolve so the shell renders
//
// NOTE: These helpers are NOT subsumed by the `wrapHandler()` middleware
// added in Task 3.2 (`lib/mocks/wrapHandler.ts`). The middleware implements
// the orthogonal `?errors=true` / `?delay=<ms>` toggles, while these helpers
// implement the per-handler `?fixture=loading|error` dispatch. The two
// systems are deliberately independent — keep both.

/**
 * Returns a promise that never resolves. Used by the `loading` fixture so
 * loading-state UIs (skeletons, spinners) stay visible during dev review.
 * Orthogonal to the `?delay=<ms>` toggle in `wrapHandler.ts`.
 */
function neverResolving<T>(): Promise<T> {
  return new Promise<T>(() => {
    /* intentionally never resolves */
  });
}

/**
 * Synthesizes a `[dev-mock] forced error` for the named command. Used by the
 * `?fixture=error` dispatch path. Orthogonal to the `?errors=true` toggle in
 * `wrapHandler.ts` — fixture-error throws even for reads, while the toggle
 * exempts reads via `isReadCommand()`.
 */
function forcedError(commandName: string): Error {
  return new Error(`[dev-mock] forced error for ${commandName}`);
}

/**
 * Standard `?fixture=error|loading` gate for profile mutators and non-shell reads.
 * Shell-critical handlers (`profile_list`, `profile_load`, …) keep bespoke logic.
 */
function withProfileFixtureGate(commandName: string, impl: Handler): Handler {
  return async (args: unknown) => {
    const fixture = getActiveFixture();
    if (fixture === 'error') throw forcedError(commandName);
    if (fixture === 'loading') return neverResolving<unknown>();
    return impl(args);
  };
}

function seedDemoProfiles(): void {
  const store = getStore();
  if (store.profiles.size > 0) return;

  const base = createDefaultProfile();

  const alpha: GameProfile = {
    ...base,
    game: {
      ...base.game,
      name: 'Test Game Alpha',
      custom_cover_art_path: '',
      custom_portrait_art_path: '',
      custom_background_art_path: '',
    },
    steam: {
      ...base.steam,
      app_id: '9999001',
    },
  };

  const beta: GameProfile = {
    ...base,
    game: {
      ...base.game,
      name: 'Dev Game Beta',
      custom_cover_art_path: '',
      custom_portrait_art_path: '',
      custom_background_art_path: '',
    },
    steam: {
      ...base.steam,
      app_id: '9999002',
    },
  };

  store.profiles.set(alpha.game.name, alpha);
  store.profiles.set(beta.game.name, beta);
  store.activeProfileId = alpha.game.name;
}

export function registerProfile(map: Map<string, Handler>): void {
  // profile_list — SHELL-CRITICAL (BR-11): always resolves with populated
  // data (or `empty` returns []) so `<AppShell />` can render under every
  // fixture state, including `error` and `loading`.
  map.set('profile_list', async () => {
    const fixture = getActiveFixture();
    if (fixture === 'empty') return [];
    seedDemoProfiles();
    return Array.from(getStore().profiles.keys());
  });

  // profile_list_summaries — SHELL-CRITICAL (BR-11): same rationale as above.
  map.set('profile_list_summaries', async (): Promise<ProfileSummary[]> => {
    const fixture = getActiveFixture();
    if (fixture === 'empty') return [];
    seedDemoProfiles();
    return Array.from(getStore().profiles.values()).map((p) => ({
      name: p.game.name,
      gameName: p.game.name,
      steamAppId: p.steam.app_id,
      customCoverArtPath: p.game.custom_cover_art_path,
      customPortraitArtPath: p.game.custom_portrait_art_path,
    }));
  });

  map.set('profile_list_favorites', async () => {
    const fixture = getActiveFixture();
    if (fixture === 'empty') return [];
    if (fixture === 'loading') return neverResolving<string[]>();
    // `error` is allowed to resolve here — favorites are non-fatal and the UI
    // can render an empty favorites strip in error state.
    seedDemoProfiles();
    return Array.from(profileFavorites).filter((n) => getStore().profiles.has(n));
  });

  map.set('profile_load', async (args) => {
    const fixture = getActiveFixture();
    if (fixture === 'empty') return null;
    if (fixture === 'loading') return neverResolving<unknown>();
    if (fixture === 'error') throw forcedError('profile_load');
    seedDemoProfiles();
    const { name } = args as { name: string };
    return getStore().profiles.get(name) ?? null;
  });

  // ── Mutation handlers ─────────────────────────────────────────────────────

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
    }),
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
    }),
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
    }),
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
    }),
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
    }),
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
    profileConfigHistory.delete(trimmed);
    if (store.activeProfileId === trimmed) {
      store.activeProfileId = store.profiles.size > 0 ? store.profiles.keys().next().value ?? null : null;
    }
    emitMockEvent('profiles-changed', { name: trimmed, action: 'delete' });
    return null;
    }),
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
    }),
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
    }),
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
    }),
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
    }),
  );

  map.set(
    'profile_export_toml',
    withProfileFixtureGate('profile_export_toml', async (args) => {
    const { name, data } = args as { name: string; data: GameProfile };
    // Return a minimal TOML-like stub for the frontend to display
    return `# CrossHook Profile Export\n# Profile: ${name}\n[game]\nname = "${data.game.name}"\nexecutable_path = "${data.game.executable_path}"\n`;
    }),
  );

  // ── Optimization preset handlers ──────────────────────────────────────────

  map.set(
    'profile_list_bundled_optimization_presets',
    withProfileFixtureGate(
      'profile_list_bundled_optimization_presets',
      async (): Promise<BundledOptimizationPreset[]> => getBundledOptimizationPresets(),
    ),
  );

  map.set(
    'profile_apply_bundled_optimization_preset',
    withProfileFixtureGate('profile_apply_bundled_optimization_preset', async (args) => {
    const { name, presetId } = args as { name: string; presetId: string };
    const trimmed = name.trim();
    const pid = presetId.trim();
    const store = getStore();
    const existing = store.profiles.get(trimmed);
    if (!existing) {
      throw new Error(`[dev-mock] profile_apply_bundled_optimization_preset: profile not found: ${trimmed}`);
    }
    const presets = getBundledOptimizationPresets();
    const normalizedPid = pid.startsWith('bundled/') ? pid : `bundled/${pid}`;
    const matched = presets.find((p) => p.preset_id === pid || p.preset_id === normalizedPid);
    const tomlKey = matched?.preset_id ?? normalizedPid;
    const presetIds = matched?.enabled_option_ids?.length ? [...matched.enabled_option_ids] : [];
    const updated: GameProfile = {
      ...existing,
      launch: {
        ...existing.launch,
        active_preset: tomlKey,
        presets: { ...(existing.launch.presets ?? {}), [tomlKey]: { enabled_option_ids: presetIds } },
        optimizations: { enabled_option_ids: presetIds },
      },
    };
    store.profiles.set(trimmed, updated);
    appendRevision(trimmed, 'preset_apply');
    emitMockEvent('profiles-changed', { name: trimmed, action: 'bundled-optimization-preset' });
    return structuredClone(updated);
    }),
  );

  map.set(
    'profile_save_manual_optimization_preset',
    withProfileFixtureGate('profile_save_manual_optimization_preset', async (args) => {
    const { name, presetName, enabledOptionIds } = args as {
      name: string;
      presetName: string;
      enabledOptionIds: string[];
    };
    const trimmed = name.trim();
    const key = presetName.trim();
    const store = getStore();
    const existing = store.profiles.get(trimmed);
    if (!existing) {
      throw new Error(`[dev-mock] profile_save_manual_optimization_preset: profile not found: ${trimmed}`);
    }
    if (!key) {
      throw new Error('[dev-mock] profile_save_manual_optimization_preset: preset name must not be empty');
    }
    const ids = [...enabledOptionIds];
    const updated: GameProfile = {
      ...existing,
      launch: {
        ...existing.launch,
        active_preset: key,
        presets: { ...(existing.launch.presets ?? {}), [key]: { enabled_option_ids: ids } },
        optimizations: { enabled_option_ids: ids },
      },
    };
    store.profiles.set(trimmed, updated);
    appendRevision(trimmed, 'preset_apply');
    emitMockEvent('profiles-changed', { name: trimmed, action: 'manual-optimization-preset' });
    return structuredClone(updated);
    }),
  );

  // ── Config history handlers ───────────────────────────────────────────────

  map.set(
    'profile_config_history',
    withProfileFixtureGate('profile_config_history', async (args): Promise<ConfigRevisionSummary[]> => {
    const { name, limit } = args as { name: string; limit?: number };
    const trimmed = name.trim();
    const rows = profileConfigHistory.get(trimmed) ?? [];
    const capped = typeof limit === 'number' ? rows.slice(0, limit) : rows;
    return structuredClone(capped);
    }),
  );

  map.set(
    'profile_config_diff',
    withProfileFixtureGate('profile_config_diff', async (args): Promise<ConfigDiffResult> => {
    const { name, revisionId, rightRevisionId } = args as {
      name: string;
      revisionId: number;
      rightRevisionId?: number;
    };
    const trimmed = name.trim();
    const rows = profileConfigHistory.get(trimmed) ?? [];
    const left = rows.find((r) => r.id === revisionId);
    if (!left) {
      throw new Error(
        `[dev-mock] profile_config_diff: revision ${revisionId} not found for profile "${trimmed}"`
      );
    }
    // In the mock, diff is always empty (no real TOML serialization)
    const rightLabel = rightRevisionId !== undefined ? `revision/${rightRevisionId}` : 'current';
    return {
      revision_id: revisionId,
      revision_source: left.source,
      revision_created_at: left.created_at,
      diff_text: `--- revision/${revisionId}\n+++ ${rightLabel}\n@@ -1,1 +1,1 @@\n [mock: no diff available in browser-dev mode]\n`,
      added_lines: 0,
      removed_lines: 0,
      truncated: false,
    };
    }),
  );

  map.set(
    'profile_config_rollback',
    withProfileFixtureGate('profile_config_rollback', async (args): Promise<ConfigRollbackResult> => {
    const { name, revisionId } = args as { name: string; revisionId: number };
    const trimmed = name.trim();
    const store = getStore();
    const existing = store.profiles.get(trimmed);
    if (!existing) {
      throw new Error(`[dev-mock] profile_config_rollback: profile not found: ${trimmed}`);
    }
    const rows = profileConfigHistory.get(trimmed) ?? [];
    const target = rows.find((r) => r.id === revisionId);
    if (!target) {
      throw new Error(
        `[dev-mock] profile_config_rollback: revision ${revisionId} not found for profile "${trimmed}"`
      );
    }
    // In the mock, rollback restores the current profile unchanged (no real TOML snapshots)
    const restored = structuredClone(existing);
    const newRevision = appendRevision(trimmed, 'rollback_apply');
    newRevision.source_revision_id = revisionId;
    emitMockEvent('profiles-changed', { name: trimmed, action: 'rollback' });
    return {
      restored_revision_id: revisionId,
      new_revision_id: newRevision.id,
      profile: restored,
    };
    }),
  );

  map.set(
    'profile_mark_known_good',
    withProfileFixtureGate('profile_mark_known_good', async (args) => {
    const { name, revisionId } = args as { name: string; revisionId: number };
    const trimmed = name.trim();
    const rows = profileConfigHistory.get(trimmed) ?? [];
    const target = rows.find((r) => r.id === revisionId);
    if (!target) {
      throw new Error(
        `[dev-mock] profile_mark_known_good: revision ${revisionId} not found for profile "${trimmed}"`
      );
    }
    // Clear known-good from all, then set on target
    for (const row of rows) {
      row.is_last_known_working = row.id === revisionId;
    }
    return null;
    }),
  );
}
