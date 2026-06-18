import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createDefaultProfile } from '@/types/profile';
import { useProfileLaunchAutosave } from '../useProfileLaunchAutosave';

// Tauri v2 exposes snake_case Rust command params to JS as camelCase. The
// command-arguments save command therefore expects `commandArguments` /
// `resolvedLaunchMethod`; sending snake_case keys makes Tauri reject the invoke
// with "missing required key commandArguments" (regression that broke every
// command-argument toggle on Steam profiles).
const callCommandMock = vi.fn();
vi.mock('@/lib/ipc', () => ({
  callCommand: (name: string, args?: unknown) => callCommandMock(name, args),
}));

function buildOptions() {
  const profile = createDefaultProfile();
  profile.launch.method = 'steam_applaunch';
  profile.steam.enabled = true;
  profile.launch.command_arguments = { enabled_argument_ids: ['force_vulkan'], custom_args: [] };
  return {
    profile,
    profileName: 'Test Profile',
    selectedProfile: 'Test Profile',
    hasExistingSavedProfile: true,
    optionsById: {},
    catalogLoaded: true,
    conflictMatrix: {},
    setProfile: vi.fn(),
    setDirty: vi.fn(),
    setError: vi.fn(),
  };
}

describe('useProfileLaunchAutosave command-arguments IPC contract', () => {
  beforeEach(() => {
    callCommandMock.mockReset();
    callCommandMock.mockImplementation((name: string) => {
      if (name === 'get_command_argument_catalog') {
        return Promise.resolve({ catalog_version: 1, entries: [] });
      }
      return Promise.resolve(undefined);
    });
  });

  it('saves command arguments with camelCase invoke keys Tauri maps to snake_case Rust params', async () => {
    // Stable options identity — rebuilding per render would re-run the autosave
    // effects on every render and loop.
    const options = buildOptions();
    const { result } = renderHook(() => useProfileLaunchAutosave(options));

    await act(async () => {
      await result.current.flushPendingLaunchSectionSaves('Test Profile');
    });

    const call = callCommandMock.mock.calls.find(([name]) => name === 'profile_save_command_arguments');
    if (!call) {
      throw new Error('expected profile_save_command_arguments to be invoked');
    }

    const args = call[1] as Record<string, unknown>;
    expect(args).toHaveProperty('resolvedLaunchMethod', 'steam_applaunch');
    expect(args).toHaveProperty('commandArguments');
    // Snake_case top-level keys would be silently dropped by Tauri → rejected invoke.
    expect(args).not.toHaveProperty('resolved_launch_method');
    expect(args).not.toHaveProperty('command_arguments');
  });
});
