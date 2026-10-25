import type { InvokeArgs } from '@tauri-apps/api/core';
import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { emitMockEvent } from '@/lib/events';
import { configureMockHandlers, mockCallCommand, resetMockHandlers } from '@/test/render';
import type { ProfilesChangedPayload } from '@/types/profile-events';
import { useCommunityProfiles } from '../useCommunityProfiles';
import { useLibrarySummaries } from '../useLibrarySummaries';
import { useProfile } from '../useProfile';

const callSpy = vi.fn((name: string, args?: InvokeArgs) => mockCallCommand(name, args));

vi.mock('@/lib/ipc', () => ({
  callCommand: (name: string, args?: InvokeArgs) => callSpy(name, args),
}));

const FAVORITE: ProfilesChangedPayload = { reason: { kind: 'favorite_toggled' }, names: ['Test Game Alpha'] };
const SAVED: ProfilesChangedPayload = { reason: { kind: 'saved' }, names: ['Test Game Alpha'] };
// Stable references: `useLibrarySummaries` re-syncs stars whenever the favorites
// array identity changes, so inline `[]` literals would re-render forever.
const NO_PROFILES: string[] = [];
const NO_FAVORITES: string[] = [];

function callsOf(command: string): number {
  return callSpy.mock.calls.filter(([name]) => name === command).length;
}

/** Crosses several macrotask boundaries so every queued refresh settles. */
async function settle(): Promise<void> {
  for (let i = 0; i < 5; i += 1) {
    const { promise, resolve } = Promise.withResolvers<void>();
    setTimeout(resolve, 0);
    await act(async () => {
      await promise;
    });
  }
}

/** Emits after the mount fetches settled and returns per-command call deltas. */
async function emitAndCount(payload: ProfilesChangedPayload, commands: string[]): Promise<Record<string, number>> {
  await settle();
  const before = Object.fromEntries(commands.map((command) => [command, callsOf(command)]));
  await act(async () => {
    emitMockEvent('profiles-changed', payload);
  });
  await settle();
  return Object.fromEntries(commands.map((command) => [command, callsOf(command) - (before[command] ?? 0)]));
}

describe('profiles-changed favorite_toggled skip', () => {
  beforeEach(() => {
    callSpy.mockClear();
    configureMockHandlers();
    vi.spyOn(console, 'debug').mockImplementation(() => {});
  });

  afterEach(() => {
    resetMockHandlers();
    vi.restoreAllMocks();
  });

  describe('useProfile', () => {
    const commands = ['profile_list', 'profile_list_favorites'];

    it('reloads only favorites for favorite_toggled', async () => {
      renderHook(() => useProfile({ autoSelectFirstProfile: false }));
      await waitFor(() => expect(callsOf('profile_list')).toBeGreaterThan(0));

      expect(await emitAndCount(FAVORITE, commands)).toEqual({ profile_list: 0, profile_list_favorites: 1 });
    });

    it('refreshes the profile list for saved (positive control)', async () => {
      renderHook(() => useProfile({ autoSelectFirstProfile: false }));
      await waitFor(() => expect(callsOf('profile_list')).toBeGreaterThan(0));

      const deltas = await emitAndCount(SAVED, commands);
      expect(deltas.profile_list).toBe(1);
      expect(deltas.profile_list_favorites).toBeGreaterThan(0);
    });
  });

  describe('useLibrarySummaries', () => {
    const commands = ['profile_list_summaries'];

    it('does not refetch summaries for favorite_toggled', async () => {
      renderHook(() => useLibrarySummaries(NO_PROFILES, NO_FAVORITES));
      await waitFor(() => expect(callsOf('profile_list_summaries')).toBe(1));

      expect(await emitAndCount(FAVORITE, commands)).toEqual({ profile_list_summaries: 0 });
    });

    it('refetches summaries for saved (positive control)', async () => {
      renderHook(() => useLibrarySummaries(NO_PROFILES, NO_FAVORITES));
      await waitFor(() => expect(callsOf('profile_list_summaries')).toBe(1));

      expect(await emitAndCount(SAVED, commands)).toEqual({ profile_list_summaries: 1 });
    });
  });

  describe('useCommunityProfiles', () => {
    const commands = ['profile_list'];

    it('does not rescan imported profile names for favorite_toggled', async () => {
      renderHook(() => useCommunityProfiles({}));
      await waitFor(() => expect(callsOf('profile_list')).toBe(1));

      expect(await emitAndCount(FAVORITE, commands)).toEqual({ profile_list: 0 });
    });

    it('rescans imported profile names for saved (positive control)', async () => {
      renderHook(() => useCommunityProfiles({}));
      await waitFor(() => expect(callsOf('profile_list')).toBe(1));

      expect(await emitAndCount(SAVED, commands)).toEqual({ profile_list: 1 });
    });
  });
});
