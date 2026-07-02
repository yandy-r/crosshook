import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createDefaultProfile, type GameProfile } from '@/types/profile';
import { useLaunchEnvironmentAutosave } from '../useLaunchEnvironmentAutosave';
import type { PersistProfileDraftResult } from '../useProfileCrud';

function buildProfile(customEnvVars: Record<string, string>): GameProfile {
  const profile = createDefaultProfile();
  profile.launch.custom_env_vars = customEnvVars;
  return profile;
}

interface HookProps {
  hasSavedSelectedProfile: boolean;
  profile: GameProfile;
  profileName: string;
}

describe('useLaunchEnvironmentAutosave', () => {
  const persistProfileDraft = vi.fn<(name: string, profile: GameProfile) => Promise<PersistProfileDraftResult>>();

  function renderAutosaveHook(initialProps: HookProps) {
    return renderHook(
      (props: HookProps) =>
        useLaunchEnvironmentAutosave({
          hasSavedSelectedProfile: props.hasSavedSelectedProfile,
          profile: props.profile,
          profileName: props.profileName,
          persistProfileDraft,
        }),
      { initialProps }
    );
  }

  async function advancePastDebounce(): Promise<void> {
    await act(async () => {
      vi.advanceTimersByTime(400);
      // Flush the persist promise so the last-persisted signature ref settles.
      await Promise.resolve();
    });
  }

  beforeEach(() => {
    vi.useFakeTimers();
    persistProfileDraft.mockReset();
    persistProfileDraft.mockResolvedValue({ ok: true });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('persists env-var edits even after live profile state already mirrors the pending edit', async () => {
    const { result, rerender } = renderAutosaveHook({
      hasSavedSelectedProfile: true,
      profile: buildProfile({}),
      profileName: 'Test Profile',
    });

    act(() => {
      result.current.handleEnvironmentBlurAutoSave('key', { key: 'FOO', value: '1' }, { FOO: '1' });
    });

    // The env-var editor pushes edits into live profile state before blur fires,
    // so by the time the debounce elapses the live profile already matches the
    // scheduled env vars. Persist must still happen.
    rerender({
      hasSavedSelectedProfile: true,
      profile: buildProfile({ FOO: '1' }),
      profileName: 'Test Profile',
    });

    await advancePastDebounce();

    expect(persistProfileDraft).toHaveBeenCalledTimes(1);
    const [name, draft] = persistProfileDraft.mock.calls[0];
    expect(name).toBe('Test Profile');
    expect(draft.launch.custom_env_vars).toEqual({ FOO: '1' });
  });

  it('persists env-var deletions', async () => {
    const { result, rerender } = renderAutosaveHook({
      hasSavedSelectedProfile: true,
      profile: buildProfile({ FOO: '1' }),
      profileName: 'Test Profile',
    });

    act(() => {
      result.current.handleEnvironmentBlurAutoSave('key', { key: 'FOO', value: '1' }, {});
    });

    rerender({
      hasSavedSelectedProfile: true,
      profile: buildProfile({}),
      profileName: 'Test Profile',
    });

    await advancePastDebounce();

    expect(persistProfileDraft).toHaveBeenCalledTimes(1);
    const [, draft] = persistProfileDraft.mock.calls[0];
    expect(draft.launch.custom_env_vars).toEqual({});
  });

  it('dedupes a second identical blur after a successful persist', async () => {
    const { result } = renderAutosaveHook({
      hasSavedSelectedProfile: true,
      profile: buildProfile({}),
      profileName: 'Test Profile',
    });

    act(() => {
      result.current.handleEnvironmentBlurAutoSave('key', { key: 'FOO', value: '1' }, { FOO: '1' });
    });
    await advancePastDebounce();

    expect(persistProfileDraft).toHaveBeenCalledTimes(1);

    act(() => {
      result.current.handleEnvironmentBlurAutoSave('key', { key: 'FOO', value: '1' }, { FOO: '1' });
    });
    await advancePastDebounce();

    expect(persistProfileDraft).toHaveBeenCalledTimes(1);
  });

  it('retries an identical blur after a failed persist', async () => {
    persistProfileDraft.mockResolvedValue({ ok: false, error: 'x' });

    const { result } = renderAutosaveHook({
      hasSavedSelectedProfile: true,
      profile: buildProfile({}),
      profileName: 'Test Profile',
    });

    act(() => {
      result.current.handleEnvironmentBlurAutoSave('key', { key: 'FOO', value: '1' }, { FOO: '1' });
    });
    await advancePastDebounce();

    expect(persistProfileDraft).toHaveBeenCalledTimes(1);

    act(() => {
      result.current.handleEnvironmentBlurAutoSave('key', { key: 'FOO', value: '1' }, { FOO: '1' });
    });
    await advancePastDebounce();

    expect(persistProfileDraft).toHaveBeenCalledTimes(2);
  });

  it('drops a scheduled save when the profile changes, then persists the new profile first blur when the edit differs from the loaded profile', async () => {
    const { result, rerender } = renderAutosaveHook({
      hasSavedSelectedProfile: true,
      profile: buildProfile({}),
      profileName: 'Old Profile',
    });

    act(() => {
      result.current.handleEnvironmentBlurAutoSave('key', { key: 'FOO', value: '1' }, { FOO: '1' });
    });

    rerender({
      hasSavedSelectedProfile: true,
      profile: buildProfile({}),
      profileName: 'New Profile',
    });

    await advancePastDebounce();

    expect(persistProfileDraft).not.toHaveBeenCalled();

    act(() => {
      result.current.handleEnvironmentBlurAutoSave('key', { key: 'FOO', value: '1' }, { FOO: '1' });
    });
    await advancePastDebounce();

    expect(persistProfileDraft).toHaveBeenCalledTimes(1);
    expect(persistProfileDraft.mock.calls[0][0]).toBe('New Profile');
  });

  it('does not persist a no-op blur after a profile switch when the edit matches the newly loaded profile', async () => {
    const { result, rerender } = renderAutosaveHook({
      hasSavedSelectedProfile: true,
      profile: buildProfile({ FOO: '1' }),
      profileName: 'Old Profile',
    });

    rerender({
      hasSavedSelectedProfile: true,
      profile: buildProfile({ FOO: '1' }),
      profileName: 'New Profile',
    });

    // Blur fires with env vars identical to New Profile's already-saved state
    // (e.g. an unrelated key/value blur that doesn't change anything).
    act(() => {
      result.current.handleEnvironmentBlurAutoSave('key', { key: 'FOO', value: '1' }, { FOO: '1' });
    });
    await advancePastDebounce();

    expect(persistProfileDraft).not.toHaveBeenCalled();

    // A subsequent blur that actually changes the env vars must still persist.
    act(() => {
      result.current.handleEnvironmentBlurAutoSave('key', { key: 'FOO', value: '1' }, { FOO: '2' });
    });
    await advancePastDebounce();

    expect(persistProfileDraft).toHaveBeenCalledTimes(1);
    expect(persistProfileDraft.mock.calls[0][0]).toBe('New Profile');
    expect(persistProfileDraft.mock.calls[0][1].launch.custom_env_vars).toEqual({ FOO: '2' });
  });

  it('does not stomp the dedupe baseline when an overlapping persist resolves out of order', async () => {
    interface Deferred {
      resolve: (value: PersistProfileDraftResult) => void;
      promise: Promise<PersistProfileDraftResult>;
    }
    function deferred(): Deferred {
      let resolve!: (value: PersistProfileDraftResult) => void;
      const promise = new Promise<PersistProfileDraftResult>((res) => {
        resolve = res;
      });
      return { resolve, promise };
    }

    const first = deferred();
    const second = deferred();
    persistProfileDraft.mockReset();
    // Third and subsequent calls resolve normally.
    persistProfileDraft.mockResolvedValue({ ok: true });
    persistProfileDraft.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);

    const { result } = renderAutosaveHook({
      hasSavedSelectedProfile: true,
      profile: buildProfile({}),
      profileName: 'Test Profile',
    });

    // Blur edit A: schedules persist 1, still unresolved when persist 2 starts.
    act(() => {
      result.current.handleEnvironmentBlurAutoSave('key', { key: 'FOO', value: 'A' }, { FOO: 'A' });
    });
    await act(async () => {
      vi.advanceTimersByTime(400);
    });
    expect(persistProfileDraft).toHaveBeenCalledTimes(1);

    // Blur edit B: schedules persist 2 while persist 1 is still in flight.
    act(() => {
      result.current.handleEnvironmentBlurAutoSave('key', { key: 'FOO', value: 'B' }, { FOO: 'B' });
    });
    await act(async () => {
      vi.advanceTimersByTime(400);
    });
    expect(persistProfileDraft).toHaveBeenCalledTimes(2);

    // Resolve persist 2 (the newer request) first, then persist 1 out of order.
    await act(async () => {
      second.resolve({ ok: true });
      await Promise.resolve();
    });
    await act(async () => {
      first.resolve({ ok: true });
      await Promise.resolve();
    });

    // Blur reverting to A's env vars must still persist — the stale resolution
    // from persist 1 must not have set the dedupe baseline back to A's signature.
    act(() => {
      result.current.handleEnvironmentBlurAutoSave('key', { key: 'FOO', value: 'A' }, { FOO: 'A' });
    });
    await advancePastDebounce();

    expect(persistProfileDraft).toHaveBeenCalledTimes(3);
    expect(persistProfileDraft.mock.calls[2][1].launch.custom_env_vars).toEqual({ FOO: 'A' });
  });

  it('does not schedule a persist when the selected profile is unsaved', async () => {
    const { result } = renderAutosaveHook({
      hasSavedSelectedProfile: false,
      profile: buildProfile({}),
      profileName: 'Test Profile',
    });

    act(() => {
      result.current.handleEnvironmentBlurAutoSave('key', { key: 'FOO', value: '1' }, { FOO: '1' });
    });
    await advancePastDebounce();

    expect(persistProfileDraft).not.toHaveBeenCalled();
  });
});
