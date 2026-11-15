import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { launchOptimizationsAutosaveDelayMs } from '@/hooks/profile/constants';
import { useProfileCrud } from '@/hooks/profile/useProfileCrud';
import { createDefaultProfile, type GameProfile } from '@/types/profile';
import { useHeroProfilesAutosave } from '../useHeroProfilesAutosave';

// Regression coverage for YAN-1987: Hero Detail autosave stopped persisting
// edits after the first save while the chip still said "Profile saved".
// The real `useProfileCrud` runs against an in-memory profile store so the
// save → reload cycle behaves like the app.

const PROFILE_NAME = 'Test Game';

type Deferred = { promise: Promise<void>; resolve: () => void };

function deferred(): Deferred {
  let resolve!: () => void;
  const promise = new Promise<void>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}

const disk = new Map<string, unknown>();
let saveGate: Deferred | null = null;
let loadGate: Deferred | null = null;
const saveCalls: unknown[] = [];

vi.mock('@/lib/ipc', () => ({
  callCommand: async (name: string, args?: Record<string, unknown>) => {
    switch (name) {
      case 'profile_list':
        return [...disk.keys()];
      case 'profile_load': {
        const gate = loadGate;
        if (gate) {
          await gate.promise;
        }
        return structuredClone(disk.get(args?.name as string));
      }
      case 'profile_save': {
        saveCalls.push(structuredClone(args?.data));
        const gate = saveGate;
        if (gate) {
          await gate.promise;
        }
        disk.set(args?.name as string, structuredClone(args?.data));
        return undefined;
      }
      default:
        return undefined;
    }
  },
}));

vi.mock('@/hooks/profile/profileMetadataSync', () => ({
  syncProfileMetadata: async () => undefined,
  clearLastUsedProfileIfDeleted: async () => undefined,
}));

const optionsById = {};
const noop = () => undefined;

function useHarness() {
  const crud = useProfileCrud({
    optionsById,
    catalogLoaded: true,
    autoSelectFirstProfile: false,
    setLastSavedProfileSnapshot: noop,
    clearAutosaveTimers: noop,
  });
  const autosave = useHeroProfilesAutosave({
    profile: crud.profile,
    profileName: crud.profileName,
    selectedProfile: crud.selectedProfile,
    profiles: crud.profiles,
    dirty: crud.dirty,
    saving: crud.saving,
    error: crud.error,
    persistProfileDraft: crud.persistProfileDraft,
    selectProfile: crud.loadProfile,
  });
  return { crud, autosave };
}

function seedProfile(): GameProfile {
  const profile = createDefaultProfile();
  profile.game.name = PROFILE_NAME;
  profile.game.executable_path = '/games/test/game.exe';
  return profile;
}

function diskAppId(): string {
  return (disk.get(PROFILE_NAME) as GameProfile).steam.app_id;
}

async function advance(ms: number) {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}

async function renderLoaded() {
  const hook = renderHook(() => useHarness());
  await act(async () => {
    await hook.result.current.crud.refreshProfiles();
  });
  await act(async () => {
    await hook.result.current.crud.loadProfile(PROFILE_NAME);
  });
  expect(hook.result.current.crud.selectedProfile).toBe(PROFILE_NAME);
  return hook;
}

function editAppId(hook: Awaited<ReturnType<typeof renderLoaded>>, appId: string) {
  act(() => {
    hook.result.current.crud.updateProfile((current) => ({
      ...current,
      steam: { ...current.steam, app_id: appId },
    }));
  });
}

describe('useHeroProfilesAutosave (YAN-1987)', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    disk.clear();
    saveCalls.length = 0;
    saveGate = null;
    loadGate = null;
    disk.set(PROFILE_NAME, seedProfile());
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('persists a second edit made after the first save, before its reload lands', async () => {
    const hook = await renderLoaded();

    // The save's post-write chain (metadata sync, list refresh, reload) is
    // several IPC round trips; hold the reload so the next edit lands inside it.
    loadGate = deferred();
    editAppId(hook, '111');
    await advance(launchOptimizationsAutosaveDelayMs);
    expect(diskAppId()).toBe('111');

    editAppId(hook, '222');
    const gate = loadGate;
    loadGate = null;
    await act(async () => {
      gate.resolve();
      await vi.advanceTimersByTimeAsync(0);
    });
    await advance(launchOptimizationsAutosaveDelayMs);

    expect(diskAppId()).toBe('222');
    expect(hook.result.current.crud.profile.steam.app_id).toBe('222');
    expect(hook.result.current.crud.dirty).toBe(false);
    expect(hook.result.current.autosave.autoSaveStatus.label).toBe('Profile saved');
  });

  it('restarts the debounce on every draft change while dirty', async () => {
    const hook = await renderLoaded();

    editAppId(hook, '1');
    await advance(launchOptimizationsAutosaveDelayMs - 50);
    editAppId(hook, '12');
    await advance(launchOptimizationsAutosaveDelayMs - 50);
    expect(saveCalls).toHaveLength(0);

    await advance(50);
    expect(saveCalls).toHaveLength(1);
    expect(diskAppId()).toBe('12');
  });

  it('saves an edit made while a save is in flight instead of overwriting it with the reload', async () => {
    const hook = await renderLoaded();

    saveGate = deferred();
    editAppId(hook, '111');
    await advance(launchOptimizationsAutosaveDelayMs);
    expect(saveCalls).toHaveLength(1);
    expect(hook.result.current.crud.saving).toBe(true);

    editAppId(hook, '222');
    const gate = saveGate;
    saveGate = null;
    await act(async () => {
      gate.resolve();
      await vi.advanceTimersByTimeAsync(0);
    });

    // The reload of the first save must not clobber the newer draft, and the
    // chip must not claim the profile is saved while disk still holds '111'.
    expect(hook.result.current.crud.profile.steam.app_id).toBe('222');
    expect(hook.result.current.autosave.autoSaveStatus.label).not.toBe('Profile saved');

    await advance(launchOptimizationsAutosaveDelayMs);
    expect(diskAppId()).toBe('222');
    expect(hook.result.current.crud.profile.steam.app_id).toBe('222');
    expect(hook.result.current.crud.dirty).toBe(false);
    expect(hook.result.current.autosave.autoSaveStatus.label).toBe('Profile saved');
  });

  it('queues a debounce that fires during a save and saves the newest draft after it', async () => {
    const hook = await renderLoaded();

    saveGate = deferred();
    editAppId(hook, '111');
    await advance(launchOptimizationsAutosaveDelayMs);
    expect(saveCalls).toHaveLength(1);

    // The next debounce fires while the first save is still held.
    editAppId(hook, '222');
    await advance(launchOptimizationsAutosaveDelayMs);
    expect(saveCalls).toHaveLength(1);

    const gate = saveGate;
    saveGate = null;
    await act(async () => {
      gate.resolve();
      await vi.advanceTimersByTimeAsync(0);
    });
    await advance(launchOptimizationsAutosaveDelayMs);

    expect(saveCalls).toHaveLength(2);
    expect(diskAppId()).toBe('222');
    expect(hook.result.current.crud.dirty).toBe(false);
    expect(hook.result.current.autosave.autoSaveStatus.label).toBe('Profile saved');
  });

  it('never shows "Profile saved" while the draft differs from disk', async () => {
    const hook = await renderLoaded();

    editAppId(hook, '111');
    await advance(launchOptimizationsAutosaveDelayMs);
    expect(hook.result.current.autosave.autoSaveStatus.label).toBe('Profile saved');

    editAppId(hook, '222');
    expect(diskAppId()).toBe('111');
    expect(hook.result.current.autosave.autoSaveStatus.label).not.toBe('Profile saved');
  });
});
