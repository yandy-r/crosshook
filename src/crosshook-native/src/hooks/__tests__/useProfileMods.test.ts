import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { DetectionScanReport, ProfileModInput, ProfileModRecord } from '@/types/mods';
import { useProfileMods } from '../useProfileMods';

const callCommandMock = vi.fn();

vi.mock('@/lib/ipc', () => ({
  callCommand: (name: string, args?: unknown) => callCommandMock(name, args),
}));

function buildMod(overrides: Partial<ProfileModRecord> = {}): ProfileModRecord {
  return {
    mod_id: 'mod-1',
    profile_id: 'profile-1',
    name: 'ReShade',
    category: 'overlay_injection',
    paths: ['dxgi.dll'],
    enabled: true,
    provenance: 'detected',
    created_at: '2026-01-01T00:00:00+00:00',
    updated_at: '2026-01-01T00:00:00+00:00',
    ...overrides,
  };
}

function buildInput(overrides: Partial<ProfileModInput> = {}): ProfileModInput {
  return {
    name: 'SKSE64',
    category: 'script_extender',
    paths: [],
    enabled: true,
    source_url: undefined,
    provenance: 'manual',
    ...overrides,
  };
}

function buildReport(overrides: Partial<DetectionScanReport> = {}): DetectionScanReport {
  return {
    scanned_root: '/games/elden-ring',
    candidates: [],
    entries_scanned: 4,
    truncated: false,
    ...overrides,
  };
}

type Handler = (args: unknown) => unknown;

function installHandlers(overrides: Record<string, Handler> = {}) {
  const defaults: Record<string, Handler> = {
    list_profile_mods: () => ({ available: true, mods: [] }),
    analyze_mod_coexistence: () => [],
  };
  callCommandMock.mockImplementation((name: string, args: unknown) => {
    const handler = overrides[name] ?? defaults[name];
    if (!handler) {
      return Promise.reject(new Error(`unexpected command: ${name}`));
    }
    try {
      return Promise.resolve(handler(args));
    } catch (error) {
      return Promise.reject(error);
    }
  });
}

function callsFor(command: string): unknown[][] {
  return callCommandMock.mock.calls.filter(([name]) => name === command);
}

describe('useProfileMods', () => {
  beforeEach(() => {
    callCommandMock.mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('returns empty without calling ipc for undefined profile', async () => {
    installHandlers();
    const { result } = renderHook(() => useProfileMods(undefined));

    await waitFor(() => expect(result.current.mods).toEqual([]));
    expect(callCommandMock).not.toHaveBeenCalled();
    expect(result.current.unavailable).toBe(false);
  });

  it('lists mods on mount with exact camelCase args', async () => {
    installHandlers();
    const { result } = renderHook(() => useProfileMods('Elden Ring'));

    await waitFor(() => expect(result.current.mods).toEqual([]));
    expect(callCommandMock.mock.calls[0]).toEqual(['list_profile_mods', { profileName: 'Elden Ring' }]);
  });

  it('exact arg keys for all six commands', async () => {
    const record = buildMod();
    installHandlers({
      add_profile_mod: () => record,
      update_profile_mod: () => record,
      remove_profile_mod: () => undefined,
      detect_profile_mods: () => buildReport(),
    });
    const { result } = renderHook(() => useProfileMods('Elden Ring'));
    await waitFor(() => expect(result.current.mods).toEqual([]));

    const input = buildInput();
    await act(async () => {
      await result.current.addMod(input);
      await result.current.updateMod('mod-1', input);
      await result.current.removeMod('mod-1');
      await result.current.detect.run();
    });
    act(() => {
      result.current.refreshAdvisories();
    });

    expect(callsFor('add_profile_mod')[0]).toEqual(['add_profile_mod', { profileName: 'Elden Ring', input }]);
    expect(callsFor('update_profile_mod')[0]).toEqual([
      'update_profile_mod',
      { profileName: 'Elden Ring', modId: 'mod-1', input },
    ]);
    expect(callsFor('remove_profile_mod')[0]).toEqual([
      'remove_profile_mod',
      { profileName: 'Elden Ring', modId: 'mod-1' },
    ]);
    expect(callsFor('detect_profile_mods')[0]).toEqual(['detect_profile_mods', { profileName: 'Elden Ring' }]);
    await waitFor(() =>
      expect(callsFor('analyze_mod_coexistence')[0]).toEqual(['analyze_mod_coexistence', { profileName: 'Elden Ring' }])
    );
  });

  it('addMod inserts the returned record locally and schedules one advisory refresh', async () => {
    installHandlers({
      add_profile_mod: () => buildMod(),
    });
    const { result } = renderHook(() => useProfileMods('Elden Ring'));
    await waitFor(() => expect(result.current.mods).toEqual([]));
    expect(callsFor('analyze_mod_coexistence')).toHaveLength(0);

    await act(async () => {
      await result.current.addMod(buildInput());
    });

    expect(callsFor('list_profile_mods')).toHaveLength(1);
    expect(result.current.mods).toEqual([buildMod()]);
    await waitFor(() => expect(callsFor('analyze_mod_coexistence')).toHaveLength(1));
  });

  it('updateMod and removeMod apply the mutation locally without re-listing', async () => {
    const record = buildMod();
    installHandlers({
      list_profile_mods: () => ({ available: true, mods: [record] }),
      update_profile_mod: () => ({ ...record, name: 'ReShade Renamed' }),
      remove_profile_mod: () => undefined,
    });
    const { result } = renderHook(() => useProfileMods('Elden Ring'));
    await waitFor(() => expect(result.current.mods).toHaveLength(1));

    await act(async () => {
      await result.current.updateMod('mod-1', buildInput());
    });
    expect(result.current.mods?.[0]?.name).toBe('ReShade Renamed');

    await act(async () => {
      await result.current.removeMod('mod-1');
    });
    expect(result.current.mods).toEqual([]);
    expect(callsFor('list_profile_mods')).toHaveLength(1);
    await waitFor(() => expect(callsFor('analyze_mod_coexistence').length).toBeGreaterThan(0));
  });

  it('toggleMod flips optimistically and reverts on error', async () => {
    installHandlers({
      list_profile_mods: () => ({ available: true, mods: [buildMod({ enabled: true })] }),
      update_profile_mod: () => {
        throw 'boom';
      },
    });
    const { result } = renderHook(() => useProfileMods('Elden Ring'));
    await waitFor(() => expect(result.current.mods).toHaveLength(1));

    let togglePromise: Promise<void> = Promise.resolve();
    act(() => {
      togglePromise = result.current.toggleMod(result.current.mods?.[0] as ProfileModRecord);
    });
    expect(result.current.mods?.[0]?.enabled).toBe(false);

    await act(async () => {
      await togglePromise;
    });
    expect(result.current.mods?.[0]?.enabled).toBe(true);
    expect(result.current.error).toBe('boom');
  });

  it('available false marks unavailable without error', async () => {
    installHandlers({
      list_profile_mods: () => ({ available: false, mods: [] }),
    });
    const { result } = renderHook(() => useProfileMods('Elden Ring'));

    await waitFor(() => expect(result.current.unavailable).toBe(true));
    expect(result.current.mods).toEqual([]);
    expect(result.current.error).toBeNull();
  });

  it('metadata_unavailable rejection marks unavailable', async () => {
    installHandlers({
      list_profile_mods: () => {
        throw 'metadata_unavailable: the metadata database could not be opened';
      },
    });
    const { result } = renderHook(() => useProfileMods('Elden Ring'));

    await waitFor(() => expect(result.current.unavailable).toBe(true));
    expect(result.current.error).toBeNull();
  });

  it('mutations short-circuit when unavailable', async () => {
    installHandlers({
      list_profile_mods: () => ({ available: false, mods: [] }),
    });
    const { result } = renderHook(() => useProfileMods('Elden Ring'));
    await waitFor(() => expect(result.current.unavailable).toBe(true));

    await expect(result.current.addMod(buildInput())).rejects.toThrow(
      'Mod registry is unavailable — the metadata database could not be opened.'
    );
    await expect(result.current.detect.run()).rejects.toThrow('Mod registry is unavailable');
    expect(callsFor('add_profile_mod')).toHaveLength(0);
    expect(callsFor('detect_profile_mods')).toHaveLength(0);
  });

  it('detect run stores report and clear discards it', async () => {
    const report = buildReport({ candidates: [], entries_scanned: 12 });
    installHandlers({
      detect_profile_mods: () => report,
    });
    const { result } = renderHook(() => useProfileMods('Elden Ring'));
    await waitFor(() => expect(result.current.mods).toEqual([]));

    await act(async () => {
      await result.current.detect.run();
    });
    expect(result.current.detect.report).toEqual(report);
    expect(result.current.detect.error).toBeNull();

    act(() => {
      result.current.detect.clear();
    });
    expect(result.current.detect.report).toBeNull();
  });

  it('detect maps no_game_path error', async () => {
    installHandlers({
      detect_profile_mods: () => {
        throw 'no_game_path: set the game executable path to enable detection';
      },
    });
    const { result } = renderHook(() => useProfileMods('Elden Ring'));
    await waitFor(() => expect(result.current.mods).toEqual([]));

    await act(async () => {
      await result.current.detect.run();
    });
    expect(result.current.detect.error).toBe('set the game executable path to enable detection');
    expect(result.current.detect.report).toBeNull();
  });

  it('refreshAdvisories debounces 300ms and coalesces calls', async () => {
    vi.useFakeTimers();
    installHandlers();
    const { result } = renderHook(() => useProfileMods('Elden Ring'));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(callsFor('analyze_mod_coexistence')).toHaveLength(0);

    act(() => {
      result.current.refreshAdvisories();
      result.current.refreshAdvisories();
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(299);
    });
    expect(callsFor('analyze_mod_coexistence')).toHaveLength(0);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1);
    });
    expect(callsFor('analyze_mod_coexistence')).toHaveLength(1);
  });

  it('cancels a pending advisory refresh when the profile switches within the debounce window', async () => {
    vi.useFakeTimers();
    const analyzedProfiles: string[] = [];
    installHandlers({
      analyze_mod_coexistence: (args) => {
        analyzedProfiles.push((args as { profileName: string }).profileName);
        return [];
      },
    });
    const { result, rerender } = renderHook(({ name }: { name: string }) => useProfileMods(name), {
      initialProps: { name: 'Profile A' },
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });

    act(() => {
      result.current.refreshAdvisories();
    });
    rerender({ name: 'Profile B' });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(400);
    });

    expect(analyzedProfiles).not.toContain('Profile A');
  });

  it('discards in-flight advisory results when the profile changed before resolve', async () => {
    vi.useFakeTimers();
    let resolveAnalyze: ((issues: unknown) => void) | undefined;
    installHandlers({
      analyze_mod_coexistence: () =>
        new Promise((resolve) => {
          resolveAnalyze = resolve;
        }),
    });
    const { result, rerender } = renderHook(({ name }: { name: string }) => useProfileMods(name), {
      initialProps: { name: 'Profile A' },
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });

    act(() => {
      result.current.refreshAdvisories();
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300);
    });
    expect(resolveAnalyze).toBeDefined();

    rerender({ name: 'Profile B' });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });
    await act(async () => {
      resolveAnalyze?.([{ message: 'stale advisory for Profile A', help: '', severity: 'warning', code: 'x' }]);
      await vi.advanceTimersByTimeAsync(0);
    });

    expect(result.current.advisories ?? []).toEqual([]);
  });

  it('advisory metadata_unavailable rejection flips unavailable', async () => {
    installHandlers({
      list_profile_mods: () => ({ available: true, mods: [buildMod()] }),
      analyze_mod_coexistence: () => {
        throw 'metadata_unavailable: the metadata database could not be opened';
      },
    });
    const { result } = renderHook(() => useProfileMods('Elden Ring'));

    await waitFor(() => expect(result.current.unavailable).toBe(true));
    expect(result.current.advisories).toBeNull();
    expect(result.current.advisoriesError).toBeNull();
  });
});
