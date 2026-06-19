import type { LutrisImportEntry, LutrisImportPreview, LutrisImportResult } from '../../../hooks/useLutrisImport';
import { createDefaultProfile } from '../../../types/profile';
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

    const results = entries.map((entry) => {
      if (!entry.importable) {
        return {
          entry,
          outcome: 'skipped' as const,
          profile_name: null,
          profile_path: null,
          error: entry.warnings[0] ?? 'Not importable',
        };
      }

      return {
        entry,
        outcome: 'imported' as const,
        profile_name: entry.suggested_name,
        profile_path: `/mock/profiles/${entry.suggested_name}.toml`,
        error: null,
      };
    });

    const imported_count = results.filter((result) => result.outcome === 'imported').length;
    const skipped_count = results.filter((result) => result.outcome === 'skipped').length;

    return {
      results,
      imported_count,
      skipped_count,
      failed_count: 0,
    };
  });
}
