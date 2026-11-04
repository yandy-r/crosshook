import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useProtonDbSuggestions } from '../useProtonDbSuggestions';

// Regression (YAN-1455): the ProtonDB card emitted accept requests with an empty
// `profileName` and nothing filled it in, so the backend rejected every
// Enable/Apply/Overwrite with "profile_name is required". The hook owns the
// profile name and must add it to the IPC request.
const callCommandMock = vi.fn();
vi.mock('@/lib/ipc', () => ({
  callCommand: (name: string, args?: unknown) => callCommandMock(name, args),
}));

const ACCEPT_RESULT = { updatedProfile: {}, appliedKeys: [], toggledOptionIds: [] };

function acceptCalls() {
  return callCommandMock.mock.calls.filter(([name]) => name === 'protondb_accept_suggestion');
}

describe('useProtonDbSuggestions acceptSuggestion', () => {
  beforeEach(() => {
    callCommandMock.mockReset();
    callCommandMock.mockImplementation((name: string) => {
      if (name === 'protondb_accept_suggestion') {
        return Promise.resolve(ACCEPT_RESULT);
      }
      return Promise.resolve(null);
    });
  });

  it('adds the profile name to catalog accept requests', async () => {
    const { result } = renderHook(() => useProtonDbSuggestions('12345', 'My Profile'));

    await act(async () => {
      await result.current.acceptSuggestion({ kind: 'catalog', catalogEntryId: 'x' });
    });

    expect(acceptCalls()).toEqual([
      ['protondb_accept_suggestion', { request: { kind: 'catalog', catalogEntryId: 'x', profileName: 'My Profile' } }],
    ]);
  });

  it('adds the profile name to env var accept requests', async () => {
    const { result } = renderHook(() => useProtonDbSuggestions('12345', 'My Profile'));

    await act(async () => {
      await result.current.acceptSuggestion({ kind: 'env_var', envKey: 'DXVK_ASYNC', envValue: '1' });
    });

    expect(acceptCalls()).toEqual([
      [
        'protondb_accept_suggestion',
        { request: { kind: 'env_var', envKey: 'DXVK_ASYNC', envValue: '1', profileName: 'My Profile' } },
      ],
    ]);
  });

  it('rejects without invoking IPC when no profile name is available', async () => {
    const { result } = renderHook(() => useProtonDbSuggestions('12345', '  '));

    await act(async () => {
      await expect(result.current.acceptSuggestion({ kind: 'catalog', catalogEntryId: 'x' })).rejects.toThrow(
        'Save the profile before accepting ProtonDB suggestions.'
      );
    });

    expect(acceptCalls()).toEqual([]);
  });

  it('does not refetch suggestions for the previous profile after the selection changes mid-accept', async () => {
    const acceptGate = Promise.withResolvers<typeof ACCEPT_RESULT>();
    callCommandMock.mockImplementation((name: string) =>
      name === 'protondb_accept_suggestion' ? acceptGate.promise : Promise.resolve(null)
    );
    const { result, rerender } = renderHook(({ profileName }) => useProtonDbSuggestions('12345', profileName), {
      initialProps: { profileName: 'Profile A' },
    });

    let pending: Promise<unknown> | undefined;
    act(() => {
      pending = result.current.acceptSuggestion({ kind: 'catalog', catalogEntryId: 'x' });
    });

    rerender({ profileName: 'Profile B' });
    callCommandMock.mockClear();

    await act(async () => {
      acceptGate.resolve(ACCEPT_RESULT);
      await pending;
    });

    const suggestionFetches = callCommandMock.mock.calls.filter(([name]) => name === 'protondb_get_suggestions');
    expect(suggestionFetches).not.toContainEqual([
      'protondb_get_suggestions',
      expect.objectContaining({ profileName: 'Profile A' }),
    ]);
  });
});
