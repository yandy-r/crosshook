import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { makeProfileDraft } from '@/test/fixtures';
import { DEFAULT_GAMESCOPE_CONFIG, type GameProfile } from '@/types/profile';
import { useLaunchSubTabsProps } from '../useLaunchSubTabsProps';

const callCommandMock = vi.fn();
vi.mock('@/lib/ipc', () => ({
  callCommand: (name: string, args?: unknown) => callCommandMock(name, args),
}));

const profileContextMock = vi.fn();
vi.mock('@/context/ProfileContext', () => ({
  useProfileContext: () => profileContextMock(),
}));

const profileHealthContext = { healthByName: {} };
vi.mock('@/context/ProfileHealthContext', () => ({
  useProfileHealthContext: () => profileHealthContext,
}));

const profile = makeProfileDraft();

function savedProfileWith(launch: Partial<GameProfile['launch']>): GameProfile {
  return { ...profile, launch: { ...profile.launch, ...launch } };
}

const CATALOG_ACCEPT_RESULT = {
  updatedProfile: savedProfileWith({ optimizations: { enabled_option_ids: ['dxvk_async'] } }),
  appliedKeys: ['DXVK_ASYNC'],
  toggledOptionIds: ['dxvk_async'],
};
const ACCEPTANCE = { kind: 'catalog', catalogEntryId: 'dxvk_async' } as const;
const ENV_ACCEPT_RESULT = {
  updatedProfile: savedProfileWith({ custom_env_vars: { PROTON_NO_ESYNC: '1' } }),
  appliedKeys: ['PROTON_NO_ESYNC'],
  toggledOptionIds: [],
};
const ENV_ACCEPTANCE = { kind: 'env_var', envKey: 'PROTON_NO_ESYNC', envValue: '1' } as const;
const IDLE_STATUS = { tone: 'idle', label: 'Ready' } as const;

const selectProfile = vi.fn().mockResolvedValue(undefined);
const persistProfileDraft = vi.fn();
const updateLaunchSetting = vi.fn<(updater: (current: GameProfile) => GameProfile) => void>();

function contextFor(selectedProfile: string, dirty: boolean, overrides: Record<string, unknown> = {}) {
  return {
    profile,
    selectedProfile,
    profileName: selectedProfile,
    dirty,
    activeCollectionId: null,
    launchMethod: 'proton_run',
    catalog: null,
    selectProfile,
    updateProfile: vi.fn(),
    updateLaunchSetting,
    persistProfileDraft,
    launchOptimizationsStatus: IDLE_STATUS,
    gamescopeAutoSaveStatus: IDLE_STATUS,
    trainerGamescopeAutoSaveStatus: IDLE_STATUS,
    mangoHudAutoSaveStatus: IDLE_STATUS,
    commandArgumentsAutoSaveStatus: IDLE_STATUS,
    ...overrides,
  };
}

function acceptCalls() {
  return callCommandMock.mock.calls.filter(([name]) => name === 'protondb_accept_suggestion');
}

/** Runs the draft updater the hook handed to `updateLaunchSetting` against `draft`. */
function applyMergedDraft(draft: GameProfile): GameProfile {
  expect(updateLaunchSetting).toHaveBeenCalledTimes(1);
  const [updater] = updateLaunchSetting.mock.calls[0];
  return updater(draft);
}

function renderSubTabsProps(hasSavedSelectedProfile = true) {
  return renderHook(() =>
    useLaunchSubTabsProps({ isGamescopeRunning: false, resolvedSteamAppId: '12345', hasSavedSelectedProfile })
  );
}

describe('useLaunchSubTabsProps onAcceptSuggestion', () => {
  beforeEach(() => {
    callCommandMock.mockReset();
    selectProfile.mockClear();
    updateLaunchSetting.mockReset();
    persistProfileDraft.mockReset();
    persistProfileDraft.mockResolvedValue({ ok: true });
    callCommandMock.mockImplementation((name: string) => {
      if (name === 'protondb_accept_suggestion') {
        return Promise.resolve(CATALOG_ACCEPT_RESULT);
      }
      return Promise.resolve(null);
    });
  });

  it('saves a dirty saved profile before accepting, then merges the accepted option', async () => {
    profileContextMock.mockReturnValue(contextFor('My Profile', true));
    const { result } = renderSubTabsProps();

    await act(async () => {
      await result.current.onAcceptSuggestion?.(ACCEPTANCE);
    });

    expect(persistProfileDraft).toHaveBeenCalledWith('My Profile', profile);
    expect(acceptCalls()).toHaveLength(1);
    const acceptIndex = callCommandMock.mock.calls.findIndex(([name]) => name === 'protondb_accept_suggestion');
    expect(persistProfileDraft.mock.invocationCallOrder[0]).toBeLessThan(
      callCommandMock.mock.invocationCallOrder[acceptIndex]
    );
    expect(applyMergedDraft(profile).launch.optimizations.enabled_option_ids).toEqual(['dxvk_async']);
    expect(selectProfile).not.toHaveBeenCalled();
  });

  it('rejects with the save error and does not accept when saving the draft fails', async () => {
    persistProfileDraft.mockResolvedValue({ ok: false, error: 'Disk is full.' });
    profileContextMock.mockReturnValue(contextFor('My Profile', true));
    const { result } = renderSubTabsProps();

    await act(async () => {
      await expect(result.current.onAcceptSuggestion?.(ACCEPTANCE)).rejects.toThrow('Disk is full.');
    });

    expect(acceptCalls()).toEqual([]);
    expect(updateLaunchSetting).not.toHaveBeenCalled();
  });

  it('refuses to accept for a dirty profile that has never been saved', async () => {
    profileContextMock.mockReturnValue(contextFor('My Profile', true));
    const { result } = renderSubTabsProps(false);

    await act(async () => {
      await expect(result.current.onAcceptSuggestion?.(ACCEPTANCE)).rejects.toThrow(
        'Save the profile before accepting ProtonDB suggestions.'
      );
    });

    expect(persistProfileDraft).not.toHaveBeenCalled();
    expect(acceptCalls()).toEqual([]);
    expect(updateLaunchSetting).not.toHaveBeenCalled();
  });

  it('refuses to accept while a launch section autosave is still pending', async () => {
    profileContextMock.mockReturnValue(
      contextFor('My Profile', false, { gamescopeAutoSaveStatus: { tone: 'saving', label: 'Saving...' } })
    );
    const { result } = renderSubTabsProps();

    await act(async () => {
      await expect(result.current.onAcceptSuggestion?.(ACCEPTANCE)).rejects.toThrow(
        'Launch settings are still saving. Try again in a moment.'
      );
    });

    expect(persistProfileDraft).not.toHaveBeenCalled();
    expect(acceptCalls()).toEqual([]);
  });

  it('merges an accepted catalog option into a clean draft without reloading the profile', async () => {
    profileContextMock.mockReturnValue(contextFor('My Profile', false));
    const { result } = renderSubTabsProps();

    await act(async () => {
      await result.current.onAcceptSuggestion?.(ACCEPTANCE);
    });

    expect(persistProfileDraft).not.toHaveBeenCalled();
    expect(acceptCalls()).toHaveLength(1);
    expect(selectProfile).not.toHaveBeenCalled();
    const merged = applyMergedDraft(savedProfileWith({ optimizations: { enabled_option_ids: ['use_gamemode'] } }));
    expect(merged.launch.optimizations.enabled_option_ids).toEqual(['use_gamemode', 'dxvk_async']);
  });

  it('leaves the draft unchanged when the catalog option was already enabled', async () => {
    callCommandMock.mockImplementation((name: string) =>
      name === 'protondb_accept_suggestion'
        ? Promise.resolve({ ...CATALOG_ACCEPT_RESULT, toggledOptionIds: [] })
        : Promise.resolve(null)
    );
    profileContextMock.mockReturnValue(contextFor('My Profile', false));
    const { result } = renderSubTabsProps();

    await act(async () => {
      await result.current.onAcceptSuggestion?.(ACCEPTANCE);
    });

    const draft = savedProfileWith({ optimizations: { enabled_option_ids: ['dxvk_async'] } });
    expect(applyMergedDraft(draft)).toBe(draft);
  });

  it('keeps draft edits made while the accept was in flight and adds only the accepted env var', async () => {
    const acceptGate = Promise.withResolvers<typeof ENV_ACCEPT_RESULT>();
    callCommandMock.mockImplementation((name: string) =>
      name === 'protondb_accept_suggestion' ? acceptGate.promise : Promise.resolve(null)
    );
    profileContextMock.mockReturnValue(contextFor('My Profile', false));
    const { result, rerender } = renderSubTabsProps();

    let pending: Promise<void> | undefined;
    act(() => {
      pending = result.current.onAcceptSuggestion?.(ENV_ACCEPTANCE);
    });

    // The user edits the draft (custom env var + gamescope) while the accept
    // IPC is still in flight; the draft is now dirty.
    const editedDraft = savedProfileWith({
      custom_env_vars: { DXVK_HUD: 'fps' },
      gamescope: { ...DEFAULT_GAMESCOPE_CONFIG, enabled: true },
    });
    profileContextMock.mockReturnValue(contextFor('My Profile', true, { profile: editedDraft }));
    rerender();

    await act(async () => {
      acceptGate.resolve(ENV_ACCEPT_RESULT);
      await pending;
    });

    expect(acceptCalls()).toHaveLength(1);
    expect(selectProfile).not.toHaveBeenCalled();
    const merged = applyMergedDraft(editedDraft);
    expect(merged.launch.custom_env_vars).toEqual({ DXVK_HUD: 'fps', PROTON_NO_ESYNC: '1' });
    expect(merged.launch.gamescope).toEqual(editedDraft.launch.gamescope);
    expect(merged.launch.optimizations).toEqual(editedDraft.launch.optimizations);
  });

  it('does not touch the draft when the selection changed during accept', async () => {
    const acceptGate = Promise.withResolvers<typeof CATALOG_ACCEPT_RESULT>();
    callCommandMock.mockImplementation((name: string) =>
      name === 'protondb_accept_suggestion' ? acceptGate.promise : Promise.resolve(null)
    );
    profileContextMock.mockReturnValue(contextFor('Profile A', false));
    const { result, rerender } = renderSubTabsProps();

    let pending: Promise<void> | undefined;
    act(() => {
      pending = result.current.onAcceptSuggestion?.(ACCEPTANCE);
    });

    profileContextMock.mockReturnValue(contextFor('Profile B', false));
    rerender();

    await act(async () => {
      acceptGate.resolve(CATALOG_ACCEPT_RESULT);
      await pending;
    });

    expect(updateLaunchSetting).not.toHaveBeenCalled();
    expect(selectProfile).not.toHaveBeenCalled();
  });
});
