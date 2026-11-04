import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { makeProfileDraft } from '@/test/fixtures';
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

const ACCEPT_RESULT = { updatedProfile: {}, appliedKeys: ['DXVK_ASYNC'], toggledOptionIds: [] };
const ACCEPTANCE = { kind: 'catalog', catalogEntryId: 'dxvk_async' } as const;
const IDLE_STATUS = { tone: 'idle', label: 'Ready' } as const;

const profile = makeProfileDraft();
const selectProfile = vi.fn().mockResolvedValue(undefined);
const persistProfileDraft = vi.fn();

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
    updateLaunchSetting: vi.fn(),
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

function renderSubTabsProps(hasSavedSelectedProfile = true) {
  return renderHook(() =>
    useLaunchSubTabsProps({ isGamescopeRunning: false, resolvedSteamAppId: '12345', hasSavedSelectedProfile })
  );
}

describe('useLaunchSubTabsProps onAcceptSuggestion', () => {
  beforeEach(() => {
    callCommandMock.mockReset();
    selectProfile.mockClear();
    persistProfileDraft.mockReset();
    persistProfileDraft.mockResolvedValue({ ok: true });
    callCommandMock.mockImplementation((name: string) => {
      if (name === 'protondb_accept_suggestion') {
        return Promise.resolve(ACCEPT_RESULT);
      }
      return Promise.resolve(null);
    });
  });

  it('saves a dirty saved profile before accepting, then reloads it', async () => {
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
    expect(selectProfile).toHaveBeenCalledWith('My Profile', { collectionId: undefined });
  });

  it('rejects with the save error and does not accept when saving the draft fails', async () => {
    persistProfileDraft.mockResolvedValue({ ok: false, error: 'Disk is full.' });
    profileContextMock.mockReturnValue(contextFor('My Profile', true));
    const { result } = renderSubTabsProps();

    await act(async () => {
      await expect(result.current.onAcceptSuggestion?.(ACCEPTANCE)).rejects.toThrow('Disk is full.');
    });

    expect(acceptCalls()).toEqual([]);
    expect(selectProfile).not.toHaveBeenCalled();
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
    expect(selectProfile).not.toHaveBeenCalled();
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

  it('accepts and reloads the profile when the draft is clean', async () => {
    profileContextMock.mockReturnValue(contextFor('My Profile', false));
    const { result } = renderSubTabsProps();

    await act(async () => {
      await result.current.onAcceptSuggestion?.(ACCEPTANCE);
    });

    expect(persistProfileDraft).not.toHaveBeenCalled();
    expect(acceptCalls()).toHaveLength(1);
    expect(selectProfile).toHaveBeenCalledWith('My Profile', { collectionId: undefined });
  });

  it('does not reload when the draft became dirty while the accept was in flight', async () => {
    const acceptGate = Promise.withResolvers<typeof ACCEPT_RESULT>();
    callCommandMock.mockImplementation((name: string) =>
      name === 'protondb_accept_suggestion' ? acceptGate.promise : Promise.resolve(null)
    );
    profileContextMock.mockReturnValue(contextFor('My Profile', false));
    const { result, rerender } = renderSubTabsProps();

    let pending: Promise<void> | undefined;
    act(() => {
      pending = result.current.onAcceptSuggestion?.(ACCEPTANCE);
    });

    profileContextMock.mockReturnValue(contextFor('My Profile', true));
    rerender();

    await act(async () => {
      acceptGate.resolve(ACCEPT_RESULT);
      await pending;
    });

    expect(acceptCalls()).toHaveLength(1);
    expect(selectProfile).not.toHaveBeenCalled();
  });

  it('does not reload the previous profile when the selection changed during accept', async () => {
    const acceptGate = Promise.withResolvers<typeof ACCEPT_RESULT>();
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
      acceptGate.resolve(ACCEPT_RESULT);
      await pending;
    });

    expect(selectProfile).not.toHaveBeenCalled();
  });
});
