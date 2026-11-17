import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { CommunityTapSubscription } from '@/types/settings';
import { useCommunityProfiles } from '../useCommunityProfiles';

const callCommandMock = vi.fn();

vi.mock('@/lib/ipc', () => ({
  callCommand: (name: string, args?: unknown) => callCommandMock(name, args),
}));

const TAP_A: CommunityTapSubscription = { url: 'https://example.invalid/a.git' };
const TAP_B: CommunityTapSubscription = { url: 'https://example.invalid/b.git', branch: 'main' };

describe('useCommunityProfiles.removeTap', () => {
  beforeEach(() => {
    callCommandMock.mockReset();
    callCommandMock.mockImplementation(async (name: string) => {
      switch (name) {
        case 'settings_load':
          return { community_taps: [TAP_A, TAP_B] };
        case 'community_list_profiles':
          return { entries: [], diagnostics: [] };
        case 'profile_list':
          return [];
        case 'community_remove_tap':
          return [TAP_B];
        default:
          throw new Error(`unexpected command ${name}`);
      }
    });
  });

  it('removes the tap through the backend service instead of rewriting settings', async () => {
    const { result } = renderHook(() => useCommunityProfiles({} as never));
    await waitFor(() => expect(result.current.taps).toHaveLength(2));

    await act(() => result.current.removeTap(TAP_A));

    expect(callCommandMock).toHaveBeenCalledWith('community_remove_tap', { tap: TAP_A });
    expect(callCommandMock).not.toHaveBeenCalledWith('settings_save', expect.anything());
    expect(result.current.taps).toEqual([TAP_B]);
  });
});
