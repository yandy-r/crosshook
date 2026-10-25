import type { LutrisImportEntry, LutrisImportPreview, LutrisImportResult } from '../../../hooks/useLutrisImport';
import { createDefaultProfile } from '../../../types/profile';
import { getStore } from '../store';
import { appendRevision } from './profile-history';
import { emitProfilesChanged } from './profile-utils';
import type { Handler } from './types';

const MOCK_LUTRIS_ROOT = '/mock/lutris';

function makeMockEntry(
  gameName: string,
  suggestedName: string,
  importable: boolean,
  warnings: string[] = []
): LutrisImportEntry {
  const profile = createDefaultProfile();
  profile.game.name = gameName;
  profile.game.executable_path = `/mock/games/${suggestedName}/game.exe`;
  profile.launch.method = 'proton_run';
  profile.runtime.prefix_path = `/mock/prefixes/${suggestedName}`;
  profile.runtime.proton_path = '/mock/compatibilitytools.d/GE-Proton9-1';

  return {
    source_path: `${MOCK_LUTRIS_ROOT}/games/${suggestedName}.yml`,
    suggested_name: suggestedName,
    game_name: gameName,
    runner: 'wine',
    mapped: profile,
    warnings,
    importable,
  };
}

const MOCK_ENTRIES: LutrisImportEntry[] = [
  makeMockEntry('Synthetic Quest', 'synthetic-quest', true),
  makeMockEntry('Dev Test Game', 'dev-test-game', true, ['Runner build not installed on host']),
  makeMockEntry('Broken Fixture', 'broken-fixture', false, ['Missing game executable']),
];

/** Mirrors core `derive_unique_profile_name`: `base`, then `base-2`, `base-3`, … */
function uniqueProfileName(base: string, taken: ReadonlySet<string>): string {
  if (!taken.has(base)) return base;
  let index = 2;
  while (taken.has(`${base}-${index}`)) index++;
  return `${base}-${index}`;
}

export function registerLutris(map: Map<string, Handler>): void {
  map.set('lutris_prepare_import', async (args): Promise<LutrisImportPreview> => {
    const { directory } = args as { directory: string | null };
    if (directory !== null && directory.trim() === '') {
      throw new Error('[dev-mock] lutris_prepare_import: directory must be a valid path when provided');
    }

    if (directory === '/mock/lutris-missing') {
      return {
        entries: [],
        lutris_root: null,
        diagnostics: ['Lutris library directory was not found.'],
      };
    }

    return {
      entries: MOCK_ENTRIES,
      lutris_root: directory ?? MOCK_LUTRIS_ROOT,
      diagnostics: [],
    };
  });

  map.set('lutris_import_profiles', async (args): Promise<LutrisImportResult> => {
    const { entries } = args as { entries: LutrisImportEntry[] };
    if (!Array.isArray(entries)) {
      throw new Error('[dev-mock] lutris_import_profiles: entries is required');
    }

    // Mirrors the backend: each importable entry is saved under a unique name,
    // then one `imported` event carries only the names actually written.
    const store = getStore();
    const results: LutrisImportResult['results'] = entries.map((entry) => {
      if (!entry.importable) {
        return {
          entry,
          outcome: 'skipped' as const,
          profile_name: null,
          profile_path: null,
          error: entry.warnings[0] ?? 'Not importable',
        };
      }

      const profileName = uniqueProfileName(entry.suggested_name, new Set(store.profiles.keys()));
      store.profiles.set(profileName, structuredClone(entry.mapped));
      appendRevision(profileName, 'import');
      return {
        entry,
        outcome: 'imported' as const,
        profile_name: profileName,
        profile_path: `/mock/profiles/${profileName}.toml`,
        error: null,
      };
    });

    const importedNames = results.flatMap((result) =>
      result.outcome === 'imported' && result.profile_name !== null ? [result.profile_name] : []
    );
    const skipped_count = results.filter((result) => result.outcome === 'skipped').length;
    if (importedNames.length > 0) {
      emitProfilesChanged({ reason: { kind: 'imported' }, names: importedNames });
    }

    return {
      results,
      imported_count: importedNames.length,
      skipped_count,
      failed_count: 0,
    };
  });
}
