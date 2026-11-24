/**
 * Parity fixtures for the Rust autosave state machine in
 * `crates/crosshook-view-model/src/profile/autosave`.
 *
 * Runs the real Classic React autosave hooks under fake timers against an
 * in-memory IPC store, and writes
 * `crates/crosshook-view-model/tests/fixtures/ts_parity/autosave.json`.
 */
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useAutoSaveChip } from '@/components/launch-subtabs/useAutoSaveChip';
import { useHeroLaunchHooksAutosave } from '@/components/library/launch/useHeroLaunchHooksAutosave';
import { useHeroProfilesAutosave } from '@/components/library/profiles/useHeroProfilesAutosave';
import { useHeroTrainerAutosave } from '@/components/library/trainer/useHeroTrainerAutosave';
import { launchOptimizationsAutosaveDelayMs } from '@/hooks/profile/constants';
import { useLaunchEnvironmentAutosave } from '@/hooks/profile/useLaunchEnvironmentAutosave';
import { useProfileCrud } from '@/hooks/profile/useProfileCrud';
import { useProfileLaunchAutosave } from '@/hooks/profile/useProfileLaunchAutosave';
import {
  DEFAULT_GAMESCOPE_CONFIG,
  DEFAULT_MANGOHUD_CONFIG,
  type GameProfile,
  type LaunchAutoSaveStatus,
} from '@/types';
import { DEFAULT_LAUNCH_COMMAND_ARGUMENTS } from '@/types/launch-command-arguments';
import { createDefaultProfile } from '@/types/profile';

const FIXTURE_FILE = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '../../../crates/crosshook-view-model/tests/fixtures/ts_parity/autosave.json'
);
const WRITE = process.env.CROSSHOOK_WRITE_PARITY_FIXTURES === '1';
if (WRITE && process.env.CI) {
  throw new Error('CROSSHOOK_WRITE_PARITY_FIXTURES must not be set in CI: it would hide fixture drift.');
}

type Deferred = { promise: Promise<void>; resolve: () => void };
type SectionKind = 'launch_optimizations' | 'gamescope' | 'trainer_gamescope' | 'mangohud' | 'command_arguments';
type LaunchMethod = 'proton_run' | 'steam_applaunch' | 'native';

function deferred(): Deferred {
  let resolve!: () => void;
  const promise = new Promise<void>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}

interface RecordedSave {
  at_ms: number;
  target: 'full_profile' | 'section';
  section_kind: SectionKind | null;
  profile_name: string;
  revision: string;
}

interface FixtureStep {
  at_ms: number;
  kind: 'edit' | 'advance_to' | 'flush' | 'hold_save' | 'release_save';
  section?: string;
  revision?: string;
}

interface FixtureScenario {
  name: string;
  scope: { selected_profile: string; draft_name: string; profile_exists: boolean; launch_method: LaunchMethod };
  steps: FixtureStep[];
  expected_saves: RecordedSave[];
  final_value: string;
}

const PROFILE_NAME = 'Test Game';
const OTHER_PROFILE = 'Other Game';
const ENVIRONMENT_AUTOSAVE_DELAY_MS = 400;
const SAVED_CHIP_HOLD_MS = 3000;

const disk = new Map<string, GameProfile>();
const recordedSaves: RecordedSave[] = [];
let saveGate: Deferred | null = null;
let startMs = 0;

const nowMs = () => Date.now() - startMs;

const SECTION_COMMANDS: Record<string, { kind: SectionKind; readRev: (args: Record<string, unknown>) => string }> = {
  profile_save_launch_optimizations: {
    kind: 'launch_optimizations',
    readRev: (a) => ((a.optimizations as { enabled_option_ids?: string[] })?.enabled_option_ids ?? []).join(','),
  },
  profile_save_gamescope_config: {
    kind: 'gamescope',
    readRev: (a) => ((a.config as { extra_args?: string[] })?.extra_args ?? []).join(','),
  },
  profile_save_trainer_gamescope_config: {
    kind: 'trainer_gamescope',
    readRev: (a) => ((a.config as { extra_args?: string[] })?.extra_args ?? []).join(','),
  },
  profile_save_mangohud_config: {
    kind: 'mangohud',
    readRev: (a) => String((a.config as { fps_limit?: number })?.fps_limit ?? ''),
  },
  profile_save_command_arguments: {
    kind: 'command_arguments',
    readRev: (a) => ((a.commandArguments as { custom_args?: string[] })?.custom_args ?? []).join(','),
  },
};

function summarizeSave(command: string, args: Record<string, unknown> = {}): RecordedSave {
  const profile_name = String(args.name ?? '');
  if (command === 'profile_save') {
    const data = args.data as GameProfile;
    let revision = data.steam.app_id || 'saved';
    if (data.pre_launch_hooks?.length) revision = data.pre_launch_hooks[0].name;
    else if (data.injection.loaded_hooks.length) revision = data.injection.loaded_hooks[0].name;
    else if (Object.keys(data.launch.custom_env_vars).length > 0)
      revision = JSON.stringify(data.launch.custom_env_vars);
    return { at_ms: nowMs(), target: 'full_profile', section_kind: null, profile_name, revision };
  }
  const entry = SECTION_COMMANDS[command];
  if (!entry) throw new Error(`Unexpected save command: ${command}`);
  return { at_ms: nowMs(), target: 'section', section_kind: entry.kind, profile_name, revision: entry.readRev(args) };
}

vi.mock('@/lib/ipc', () => ({
  callCommand: async (name: string, args?: Record<string, unknown>) => {
    if (name === 'profile_list') return [...disk.keys()];
    if (name === 'profile_load') return structuredClone(disk.get(args?.name as string));
    if (name === 'get_command_argument_catalog') return { catalog_version: 1, entries: [] };
    if (name === 'profile_list_bundled_optimization_presets') return [];
    if (name === 'profile_save') {
      recordedSaves.push(summarizeSave(name, args));
      if (saveGate) await saveGate.promise;
      disk.set(args?.name as string, structuredClone(args?.data as GameProfile));
    } else if (name in SECTION_COMMANDS) {
      recordedSaves.push(summarizeSave(name, args));
    }
    return undefined;
  },
}));

vi.mock('@/hooks/profile/profileMetadataSync', () => ({
  syncProfileMetadata: async () => undefined,
  clearLastUsedProfileIfDeleted: async () => undefined,
}));

function makeSeedProfile(method: LaunchMethod = 'proton_run'): GameProfile {
  const profile = createDefaultProfile();
  profile.game.name = PROFILE_NAME;
  profile.game.executable_path = method === 'native' ? '/usr/bin/game' : '/games/test/game.exe';
  profile.launch.method = method;
  profile.steam.enabled = method === 'steam_applaunch';
  profile.launch.command_arguments = { ...DEFAULT_LAUNCH_COMMAND_ARGUMENTS };
  return profile;
}

const emptyObj = {};
const noop = () => undefined;

function useFullHarness(draftNameOverride?: string) {
  const crud = useProfileCrud({
    optionsById: emptyObj,
    catalogLoaded: false,
    autoSelectFirstProfile: false,
    setLastSavedProfileSnapshot: noop,
    clearAutosaveTimers: noop,
  });
  const draftName = draftNameOverride ?? crud.profileName;
  const heroAutosave = useHeroProfilesAutosave({
    profile: crud.profile,
    profileName: draftName,
    selectedProfile: crud.selectedProfile,
    profiles: crud.profiles,
    dirty: crud.dirty,
    saving: crud.saving,
    error: crud.error,
    persistProfileDraft: crud.persistProfileDraft,
    selectProfile: crud.loadProfile,
  });
  const hasExisting =
    crud.selectedProfile.trim().length > 0 &&
    crud.profiles.includes(crud.selectedProfile.trim()) &&
    draftName.trim() === crud.selectedProfile.trim();
  const launchAutosave = useProfileLaunchAutosave({
    profile: crud.profile,
    profileName: draftName,
    selectedProfile: crud.selectedProfile,
    hasExistingSavedProfile: hasExisting,
    optionsById: emptyObj,
    catalogLoaded: false,
    conflictMatrix: emptyObj,
    setProfile: crud.setProfile,
    setDirty: crud.setDirty,
    setError: crud.setError,
  });
  const common = {
    hasSavedSelectedProfile: hasExisting,
    profile: crud.profile,
    profileName: draftName,
    persistProfileDraft: crud.persistProfileDraft,
  };
  const envAutosave = useLaunchEnvironmentAutosave(common);
  const hooksAutosave = useHeroLaunchHooksAutosave(common);
  const trainerAutosave = useHeroTrainerAutosave(common);
  return { crud, heroAutosave, launchAutosave, envAutosave, hooksAutosave, trainerAutosave };
}

type HarnessRef = { readonly current: ReturnType<typeof useFullHarness> };

async function setupHarness(options?: { method?: LaunchMethod; saved?: boolean; draftNameOverride?: string }) {
  const method = options?.method ?? 'proton_run';
  const saved = options?.saved ?? true;
  disk.clear();
  recordedSaves.length = 0;
  saveGate = null;
  vi.setSystemTime(0);
  startMs = Date.now();
  if (saved) {
    disk.set(PROFILE_NAME, makeSeedProfile(method));
    disk.set(OTHER_PROFILE, makeSeedProfile(method));
  }
  const hook = renderHook(() => useFullHarness(options?.draftNameOverride));
  await act(async () => {
    await hook.result.current.crud.refreshProfiles();
  });
  if (saved) {
    await act(async () => {
      await hook.result.current.crud.loadProfile(PROFILE_NAME);
    });
  } else {
    act(() => {
      hook.result.current.crud.setProfile(makeSeedProfile(method));
      hook.result.current.crud.setProfileName('Unsaved Draft');
    });
  }
  recordedSaves.length = 0;
  vi.setSystemTime(0);
  startMs = Date.now();
  return hook;
}

async function advanceTo(targetMs: number, steps?: FixtureStep[], recordStep = false) {
  const delta = targetMs - nowMs();
  if (delta > 0)
    await act(async () => {
      await vi.advanceTimersByTimeAsync(delta);
    });
  if (recordStep && steps) steps.push({ at_ms: targetMs, kind: 'advance_to' });
}

async function runScenario(
  name: string,
  options: { method?: LaunchMethod; saved?: boolean; draftNameOverride?: string } | undefined,
  body: (r: HarnessRef, steps: FixtureStep[]) => Promise<void>,
  finalValue: () => string = () => recordedSaves.at(-1)?.revision ?? ''
): Promise<FixtureScenario> {
  const method = options?.method ?? 'proton_run';
  const saved = options?.saved ?? true;
  const hook = await setupHarness(options);
  const steps: FixtureStep[] = [];
  await body(hook.result, steps);
  hook.unmount();
  return {
    name,
    scope: {
      selected_profile: saved ? PROFILE_NAME : '',
      draft_name: options?.draftNameOverride ?? (saved ? PROFILE_NAME : 'Unsaved Draft'),
      profile_exists: saved,
      launch_method: method,
    },
    steps,
    expected_saves: [...recordedSaves],
    final_value: finalValue(),
  };
}

const setAppId = (r: HarnessRef, val: string) =>
  act(() => r.current.crud.updateProfile((p) => ({ ...p, steam: { ...p.steam, app_id: val } })));
const diskAppId = () => disk.get(PROFILE_NAME)?.steam.app_id ?? '';
const setGamescope = (r: HarnessRef, key: 'gamescope' | 'trainer_gamescope', arg: string) =>
  act(() =>
    r.current.crud.updateLaunchSetting((p) => ({
      ...p,
      launch: { ...p.launch, [key]: { ...DEFAULT_GAMESCOPE_CONFIG, enabled: true, extra_args: [arg] } },
    }))
  );
const setMangohud = (r: HarnessRef, fps: number) =>
  act(() =>
    r.current.crud.updateLaunchSetting((p) => ({
      ...p,
      launch: { ...p.launch, mangohud: { ...DEFAULT_MANGOHUD_CONFIG, enabled: true, fps_limit: fps } },
    }))
  );
const stepEdit = (s: FixtureStep[], at: number, section: string, revision: string) =>
  s.push({ at_ms: at, kind: 'edit', section, revision });

async function runScenarios(): Promise<FixtureScenario[]> {
  return [
    await runScenario(
      'rapid_profile_draft_edits_coalesce',
      undefined,
      async (r, s) => {
        for (const [at, val] of [
          [0, '111'],
          [150, '222'],
          [300, '333'],
        ] as const) {
          await advanceTo(at);
          setAppId(r, val);
          stepEdit(s, at, 'profile_draft', val);
        }
        await advanceTo(650, s, true);
      },
      diskAppId
    ),
    await runScenario('rapid_launch_optimizations_coalesce', undefined, async (r, s) => {
      act(() => r.current.launchAutosave.toggleLaunchOptimization('opt_a', true));
      stepEdit(s, 0, 'launch_optimizations', 'opt_a');
      await advanceTo(150);
      act(() => r.current.launchAutosave.toggleLaunchOptimization('opt_b', true));
      stepEdit(s, 150, 'launch_optimizations', 'opt_a,opt_b');
      await advanceTo(300);
      act(() => r.current.launchAutosave.toggleLaunchOptimization('opt_a', false));
      stepEdit(s, 300, 'launch_optimizations', 'opt_b');
      await advanceTo(650, s, true);
    }),
    await runScenario('separate_launch_sections_sequential', undefined, async (r, s) => {
      setGamescope(r, 'gamescope', 'gs-1');
      stepEdit(s, 0, 'gamescope', 'gs-1');
      await advanceTo(350, s, true);
      await advanceTo(400);
      setMangohud(r, 60);
      stepEdit(s, 400, 'mangohud', '60');
      await advanceTo(750, s, true);
    }),
    await runScenario('all_five_launch_sections_dedicated_targets', undefined, async (r, s) => {
      act(() => r.current.launchAutosave.toggleLaunchOptimization('gamemode', true));
      stepEdit(s, 0, 'launch_optimizations', 'gamemode');
      await advanceTo(400, s, true);
      setGamescope(r, 'gamescope', 'gs');
      stepEdit(s, 400, 'gamescope', 'gs');
      await advanceTo(800, s, true);
      setGamescope(r, 'trainer_gamescope', 'tgs');
      stepEdit(s, 800, 'trainer_gamescope', 'tgs');
      await advanceTo(1200, s, true);
      setMangohud(r, 90);
      stepEdit(s, 1200, 'mangohud', '90');
      await advanceTo(1600, s, true);
      act(() => r.current.launchAutosave.updateCommandArgumentsCustomArgs(['--fast']));
      stepEdit(s, 1600, 'command_arguments', '--fast');
      await advanceTo(2000, s, true);
    }),
    await runScenario(
      'environment_blur_400ms_and_dedupe',
      undefined,
      async (r, s) => {
        for (const at of [0, 500]) {
          await advanceTo(at);
          act(() =>
            r.current.envAutosave.handleEnvironmentBlurAutoSave('key', { key: 'FOO', value: '1' }, { FOO: '1' })
          );
          stepEdit(s, at, 'environment', '{"FOO":"1"}');
          if (at === 0) {
            await advanceTo(350, s, true);
            await advanceTo(400, s, true);
          }
        }
        await advanceTo(1000, s, true);
      },
      () => JSON.stringify(disk.get(PROFILE_NAME)?.launch.custom_env_vars ?? {})
    ),
    await runScenario(
      'launch_hooks_350ms_full_profile',
      undefined,
      async (r, s) => {
        const next: GameProfile = {
          ...r.current.crud.profile,
          pre_launch_hooks: [{ id: 'h1', name: 'hook-1', path: '/bin/true', stage: 'pre-launch', enabled: true }],
        };
        act(() => {
          r.current.crud.setProfile(next);
          r.current.hooksAutosave.scheduleHookAutosave(next);
        });
        stepEdit(s, 0, 'launch_hooks', 'hook-1');
        await advanceTo(350, s, true);
      },
      () => disk.get(PROFILE_NAME)?.pre_launch_hooks?.[0]?.name ?? ''
    ),
    await runScenario(
      'trainer_350ms_full_profile',
      undefined,
      async (r, s) => {
        const next: GameProfile = {
          ...r.current.crud.profile,
          injection: {
            ...r.current.crud.profile.injection,
            loaded_hooks: [{ id: 't1', name: 'trainer-1', path: '/tmp/a.dll', enabled: true }],
          },
        };
        act(() => {
          r.current.crud.setProfile(next);
          r.current.trainerAutosave.scheduleTrainerAutosave(next);
        });
        stepEdit(s, 0, 'trainer', 'trainer-1');
        await advanceTo(350, s, true);
      },
      () => disk.get(PROFILE_NAME)?.injection.loaded_hooks[0]?.name ?? ''
    ),
    await runScenario(
      'flush_on_card_switch_saves_once',
      undefined,
      async (r, s) => {
        setAppId(r, '555');
        await advanceTo(100);
        await act(async () => {
          await r.current.heroAutosave.selectCard(OTHER_PROFILE);
        });
        stepEdit(s, 0, 'profile_draft', '555');
        s.push({ at_ms: 100, kind: 'flush' });
        await advanceTo(600, s, true);
      },
      diskAppId
    ),
    await runScenario('flush_launch_sections_saves_once', undefined, async (r, s) => {
      act(() => {
        r.current.launchAutosave.toggleLaunchOptimization('opt_x', true);
        r.current.launchAutosave.updateCommandArgumentsCustomArgs(['--arg']);
      });
      await advanceTo(100);
      await act(async () => {
        await r.current.launchAutosave.flushPendingLaunchSectionSaves(PROFILE_NAME);
      });
      stepEdit(s, 0, 'launch_optimizations', 'opt_x');
      stepEdit(s, 0, 'command_arguments', '--arg');
      s.push({ at_ms: 100, kind: 'flush' });
      await advanceTo(600, s, true);
    }),
    await runScenario('native_method_gates_optimizations_and_command_arguments', { method: 'native' }, async (r, s) => {
      act(() => {
        r.current.launchAutosave.toggleLaunchOptimization('opt_a', true);
        r.current.launchAutosave.updateCommandArgumentsCustomArgs(['--native']);
      });
      stepEdit(s, 0, 'launch_optimizations', 'opt_a');
      stepEdit(s, 0, 'command_arguments', '--native');
      await advanceTo(500, s, true);
    }),
    await runScenario('unsaved_profile_suppresses_autosave', { saved: false }, async (r, s) => {
      setAppId(r, '777');
      stepEdit(s, 0, 'profile_draft', '777');
      await advanceTo(500, s, true);
    }),
    await runScenario('mid_rename_suppresses_autosave', { draftNameOverride: 'Renamed Draft' }, async (r, s) => {
      setAppId(r, '888');
      stepEdit(s, 0, 'profile_draft', '888');
      await advanceTo(500, s, true);
    }),
    await runScenario(
      'edit_during_in_flight_save_persists_latest',
      undefined,
      async (r, s) => {
        saveGate = deferred();
        setAppId(r, '111');
        stepEdit(s, 0, 'profile_draft', '111');
        s.push({ at_ms: 0, kind: 'hold_save' });
        await advanceTo(350, s, true);
        await advanceTo(400);
        setAppId(r, '222');
        stepEdit(s, 400, 'profile_draft', '222');
        await advanceTo(450);
        const gate = saveGate;
        saveGate = null;
        await act(async () => {
          gate?.resolve();
          await vi.advanceTimersByTimeAsync(0);
        });
        s.push({ at_ms: 450, kind: 'release_save' });
        await advanceTo(750, s, true);
      },
      diskAppId
    ),
  ];
}

async function runChipCases() {
  const st = (tone: LaunchAutoSaveStatus['tone'], label: string): LaunchAutoSaveStatus => ({ tone, label });
  const [idle, saved, warn, saving, err] = [
    st('idle', 'Ready'),
    st('success', 'Saved automatically'),
    st('warning', 'Save profile first'),
    st('saving', 'Saving...'),
    st('error', 'Failed to save'),
  ];
  const inputs = [
    { name: 'error_beats_all_other_tones', statuses: [idle, saved, warn, saving, err] },
    { name: 'saving_beats_warning_and_success', statuses: [saved, warn, saving] },
    {
      name: 'warning_tie_keeps_first_label',
      statuses: [idle, st('warning', 'First warning'), st('warning', 'Second warning'), saved],
    },
    { name: 'success_beats_idle_and_fades_at_3000ms', statuses: [idle, saved] },
  ];
  const tick = (ms: number) =>
    act(async () => {
      await vi.advanceTimersByTimeAsync(ms);
    });
  const out = [];
  for (const { name, statuses } of inputs) {
    const hook = renderHook(() =>
      useAutoSaveChip({
        launchOptimizationsStatus: statuses[0],
        gamescopeAutoSaveStatus: statuses[1],
        mangoHudAutoSaveStatus: statuses[2],
        commandArgumentsAutoSaveStatus: statuses[4] ?? statuses[3],
      })
    );
    const { tone: expected_tone, label: expected_label } = hook.result.current.combinedAutoSaveStatus;
    await tick(SAVED_CHIP_HOLD_MS - 1);
    const visible_at_2999ms = hook.result.current.chipVisible;
    await tick(1);
    out.push({
      name,
      statuses,
      expected_tone,
      expected_label,
      visible_at_2999ms,
      visible_at_3000ms: hook.result.current.chipVisible,
    });
    hook.unmount();
  }
  return out;
}

describe('TS parity fixtures for crosshook-view-model profile autosave', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('matches Classic autosave hooks across all debounce, grouping, flush, gating and chip scenarios', async () => {
    const text = `${JSON.stringify({ constants: { section_debounce_ms: launchOptimizationsAutosaveDelayMs, environment_debounce_ms: ENVIRONMENT_AUTOSAVE_DELAY_MS, saved_status_hold_ms: SAVED_CHIP_HOLD_MS }, scenarios: await runScenarios(), chip_cases: await runChipCases() }, null, 2)}\n`;
    if (WRITE) {
      mkdirSync(path.dirname(FIXTURE_FILE), { recursive: true });
      writeFileSync(FIXTURE_FILE, text);
      return;
    }
    expect(JSON.parse(readFileSync(FIXTURE_FILE, 'utf8'))).toEqual(JSON.parse(text));
  });
});
