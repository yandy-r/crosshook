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
const UNSAVED_MESSAGE = 'Save or discard your unsaved profile changes before applying ProtonDB suggestions.';

const profile = makeProfileDraft();
const selectProfile = vi.fn().mockResolvedValue(undefined);

function contextFor(selectedProfile: string, dirty: boolean) {
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
    persistProfileDraft: vi.fn(),
  };
}

function acceptCalls() {
  return callCommandMock.mock.calls.filter(([name]) => name === 'protondb_accept_suggestion');
}

function renderSubTabsProps() {
  return renderHook(() =>
    useLaunchSubTabsProps({ isGamescopeRunning: false, resolvedSteamAppId: '12345', hasSavedSelectedProfile: true })
  );
}

describe('useLaunchSubTabsProps onAcceptSuggestion', () => {
  beforeEach(() => {
    callCommandMock.mockReset();
    selectProfile.mockClear();
    callCommandMock.mockImplementation((name: string) => {
      if (name === 'protondb_accept_suggestion') {
        return Promise.resolve(ACCEPT_RESULT);
      }
      return Promise.resolve(null);
    });
  });

  it('refuses to accept while the profile draft has unsaved changes', async () => {
    profileContextMock.mockReturnValue(contextFor('My Profile', true));
    const { result } = renderSubTabsProps();

    await act(async () => {
      await expect(
        result.current.onAcceptSuggestion?.({ kind: 'catalog', catalogEntryId: 'dxvk_async' })
      ).rejects.toThrow(UNSAVED_MESSAGE);
    });

    expect(acceptCalls()).toEqual([]);
    expect(selectProfile).not.toHaveBeenCalled();
  });

  it('accepts and reloads the profile when the draft is clean', async () => {
    profileContextMock.mockReturnValue(contextFor('My Profile', false));
    const { result } = renderSubTabsProps();

    await act(async () => {
      await result.current.onAcceptSuggestion?.({ kind: 'catalog', catalogEntryId: 'dxvk_async' });
    });

    expect(acceptCalls()).toHaveLength(1);
    expect(selectProfile).toHaveBeenCalledWith('My Profile', { collectionId: undefined });
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
      pending = result.current.onAcceptSuggestion?.({ kind: 'catalog', catalogEntryId: 'dxvk_async' });
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
