// Mock IPC handlers for the mod coexistence registry commands.
// See `lib/mocks/README.md`. All error messages MUST start with `[dev-mock]`
// unless they reproduce a documented backend error-string contract
// (`metadata_unavailable:` / `no_game_path:` prefixes, duplicate-name text).

import type { LaunchValidationIssue } from '../../../types/launch';
import type {
  DetectionScanReport,
  ModCategory,
  ModProvenance,
  ProfileModInput,
  ProfileModRecord,
  ProfileModsResponse,
} from '../../../types/mods';
import { getStore } from '../store';
import type { Handler } from './types';

const SEED_PROFILE = 'Test Game Alpha';

/** Toggle from the console to exercise the degraded-registry UI. */
let modsUnavailable = false;
let nextId = 1;
let registry = new Map<string, ProfileModRecord[]>();

function nowIso(): string {
  return new Date().toISOString();
}

function makeRecord(
  profileName: string,
  name: string,
  category: ModCategory,
  provenance: ModProvenance,
  paths: string[],
  enabled = true
): ProfileModRecord {
  const stamp = nowIso();
  return {
    mod_id: `mock-mod-${nextId++}`,
    profile_id: `mock-profile-${profileName.toLowerCase().replace(/\s+/g, '-')}`,
    name,
    category,
    paths,
    enabled,
    provenance,
    created_at: stamp,
    updated_at: stamp,
  };
}

function seedRegistry(): Map<string, ProfileModRecord[]> {
  return new Map([
    [
      SEED_PROFILE,
      [
        makeRecord(SEED_PROFILE, 'ReShade', 'overlay_injection', 'detected', ['dxgi.dll', 'ReShade.ini']),
        makeRecord(SEED_PROFILE, 'HD texture pack', 'other', 'manual', []),
      ],
    ],
  ]);
}

registry = seedRegistry();

export function resetModsMockState(): void {
  modsUnavailable = false;
  nextId = 1;
  registry = seedRegistry();
}

function modsFor(profileName: string): ProfileModRecord[] {
  const existing = registry.get(profileName);
  if (existing) {
    return existing;
  }
  const fresh: ProfileModRecord[] = [];
  registry.set(profileName, fresh);
  return fresh;
}

function requireProfileName(args: unknown): string {
  const profileName = (args as { profileName?: unknown })?.profileName;
  if (typeof profileName !== 'string' || profileName.trim().length === 0) {
    throw new Error('[dev-mock] profileName is required');
  }
  return profileName.trim();
}

function assertAvailable(): void {
  if (modsUnavailable) {
    throw new Error('metadata_unavailable: the metadata database could not be opened');
  }
}

function normalizeInput(raw: unknown): ProfileModInput {
  const input = raw as ProfileModInput;
  if (!input || typeof input.name !== 'string' || input.name.trim().length === 0) {
    throw new Error('metadata validation error: mod name is required');
  }
  return {
    name: input.name.trim(),
    category: input.category,
    paths: (input.paths ?? []).map((p) => p.trim()).filter((p) => p.length > 0),
    enabled: input.enabled ?? true,
    source_url: input.source_url?.trim() ? input.source_url.trim() : undefined,
    provenance: input.provenance,
  };
}

function gameExecutablePath(profileName: string): string {
  const profile = getStore().profiles.get(profileName);
  return profile?.game.executable_path?.trim() ?? '';
}

function makeCannedReport(): DetectionScanReport {
  return {
    scanned_root: '/games/mock-game',
    candidates: [
      {
        detector_id: 'reshade',
        suggested_name: 'ReShade',
        category: 'overlay_injection',
        matched_paths: ['ReShade.ini', 'dxgi.dll', 'reshade-shaders'],
        already_registered: true,
      },
      {
        detector_id: 'script_extender:skse64',
        suggested_name: 'SKSE64',
        category: 'script_extender',
        matched_paths: ['Data/SKSE', 'skse64_1_6_1170.dll', 'skse64_loader.exe'],
        already_registered: false,
      },
    ],
    entries_scanned: 118,
    truncated: false,
  };
}

function cannedAdvisories(mods: ProfileModRecord[]): LaunchValidationIssue[] {
  const relevant = mods.filter(
    (mod) => mod.enabled && (mod.category === 'overlay_injection' || mod.category === 'script_extender')
  );
  if (relevant.length === 0) {
    return [];
  }
  const first = relevant[0];
  return [
    {
      message: `Mod “${first.name}” and the configured trainer rely on the same in-process load vector.`,
      help: "Proxy-DLL loaders and script extenders hook the game process the same way CrossHook's injection/copy-to-prefix trainer paths do. If the game crashes at startup, disable one side or switch the trainer loading mode to source_directory.",
      severity: 'warning',
      code: 'mod_coexistence_injection_vector',
    },
    {
      message: `File-replacement mod “${first.name}” may shadow hash baselines.`,
      help: 'Hash baselines and detection scans may reflect modded files instead of vanilla ones. Launch is not blocked.',
      severity: 'info',
      code: 'mod_coexistence_file_replacement_notice',
    },
  ];
}

export function registerMods(map: Map<string, Handler>): void {
  map.set('list_profile_mods', async (args): Promise<ProfileModsResponse> => {
    const profileName = requireProfileName(args);
    if (modsUnavailable) {
      return { available: false, mods: [] };
    }
    return { available: true, mods: [...modsFor(profileName)] };
  });

  map.set('add_profile_mod', async (args): Promise<ProfileModRecord> => {
    assertAvailable();
    const profileName = requireProfileName(args);
    const input = normalizeInput((args as { input?: unknown }).input);
    const mods = modsFor(profileName);
    if (mods.some((mod) => mod.name.toLowerCase() === input.name.toLowerCase())) {
      throw new Error(`metadata validation error: a mod named “${input.name}” is already registered for this profile`);
    }
    const record: ProfileModRecord = {
      ...makeRecord(profileName, input.name, input.category, input.provenance, input.paths, input.enabled),
      source_url: input.source_url,
    };
    mods.push(record);
    return { ...record };
  });

  map.set('update_profile_mod', async (args): Promise<ProfileModRecord> => {
    assertAvailable();
    const profileName = requireProfileName(args);
    const modId = (args as { modId?: unknown }).modId;
    const input = normalizeInput((args as { input?: unknown }).input);
    const mods = modsFor(profileName);
    const index = mods.findIndex((mod) => mod.mod_id === modId);
    if (index < 0) {
      throw new Error('metadata validation error: mod not found');
    }
    const updated: ProfileModRecord = {
      ...mods[index],
      name: input.name,
      category: input.category,
      paths: input.paths,
      enabled: input.enabled,
      source_url: input.source_url,
      provenance: input.provenance,
      updated_at: nowIso(),
    };
    mods[index] = updated;
    return { ...updated };
  });

  map.set('remove_profile_mod', async (args): Promise<void> => {
    assertAvailable();
    const profileName = requireProfileName(args);
    const modId = (args as { modId?: unknown }).modId;
    const mods = modsFor(profileName);
    const index = mods.findIndex((mod) => mod.mod_id === modId);
    if (index < 0) {
      throw new Error('mod not found');
    }
    mods.splice(index, 1);
  });

  map.set('detect_profile_mods', async (args): Promise<DetectionScanReport> => {
    const profileName = requireProfileName(args);
    if (gameExecutablePath(profileName).length === 0) {
      throw new Error('no_game_path: set the game executable path to enable detection');
    }
    const report = makeCannedReport();
    if (modsUnavailable) {
      for (const candidate of report.candidates) {
        candidate.already_registered = false;
      }
      return report;
    }
    const mods = modsFor(profileName);
    for (const candidate of report.candidates) {
      candidate.already_registered = mods.some(
        (mod) => mod.name.toLowerCase() === candidate.suggested_name.toLowerCase()
      );
    }
    return report;
  });

  map.set('analyze_mod_coexistence', async (args): Promise<LaunchValidationIssue[]> => {
    assertAvailable();
    const profileName = requireProfileName(args);
    return cannedAdvisories(modsFor(profileName));
  });
}
