import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { makeProfileDraft } from '@/test/fixtures';
import { useProfileActions } from '../useProfileActions';

// Tauri v2 reads top-level invoke keys as the lowerCamelCase form of the Rust
// params. Snake_case keys are dropped: a required param rejects the invoke
// ("missing required key profileName"), an Option param silently becomes None
// (so "include hooks" exported without hooks).
const callCommandMock = vi.fn();
vi.mock('@/lib/ipc', () => ({
  callCommand: (name: string, args?: unknown) => callCommandMock(name, args),
}));

const chooseSaveFileMock = vi.fn();
vi.mock('@/utils/dialog', () => ({
  chooseSaveFile: (...args: unknown[]) => chooseSaveFileMock(...args),
}));

// Stable identities: a fresh context object per render would re-run the
// hook's callbacks/effects on every render.
const mockProfileContext = {
  deleting: false,
  dirty: false,
  duplicateProfile: vi.fn(),
  duplicating: false,
  loading: false,
  pendingDelete: null,
  profile: makeProfileDraft(),
  profileExists: true,
  profileName: 'Test Profile',
  profiles: ['Test Profile'],
  refreshProfiles: vi.fn(),
  renameProfile: vi.fn(),
  renaming: false,
  saveProfile: vi.fn(),
  saving: false,
  selectedProfile: 'Test Profile',
};
const mockProfileHealthContext = { batchValidate: vi.fn() };

vi.mock('@/context/ProfileContext', () => ({
  useProfileContext: () => mockProfileContext,
}));

vi.mock('@/context/ProfileHealthContext', () => ({
  useProfileHealthContext: () => mockProfileHealthContext,
}));

function findCall(name: string): Record<string, unknown> {
  const call = callCommandMock.mock.calls.find(([command]) => command === name);
  if (!call) {
    throw new Error(`expected ${name} to be invoked`);
  }
  return call[1] as Record<string, unknown>;
}

describe('useProfileActions IPC contract', () => {
  beforeEach(() => {
    callCommandMock.mockReset();
    chooseSaveFileMock.mockReset();
  });

  it('exports a community profile with camelCase profileName / outputPath keys', async () => {
    chooseSaveFileMock.mockResolvedValue('/tmp/test-profile.json');
    callCommandMock.mockResolvedValue({
      profile_name: 'Test Profile',
      output_path: '/tmp/test-profile.json',
      manifest: {},
    });
    const { result } = renderHook(() => useProfileActions({ setPendingLauncherReExport: vi.fn() }));

    await act(async () => {
      await result.current.handleExportCommunityProfile();
    });

    const args = findCall('community_export_profile');
    expect(args).toHaveProperty('profileName', 'Test Profile');
    expect(args).toHaveProperty('outputPath', '/tmp/test-profile.json');
    expect(args).not.toHaveProperty('profile_name');
    expect(args).not.toHaveProperty('output_path');
    expect(result.current.communityExportSuccess).toBe('Community profile saved to /tmp/test-profile.json');
  });

  it.each([
    ['handlePreviewProfile', false],
    ['handleIncludeHooksInPreview', true],
  ] as const)('%s sends includeHooks=%s as a camelCase key', async (handler, includeHooks) => {
    callCommandMock.mockResolvedValue('# toml');
    const { result } = renderHook(() => useProfileActions({ setPendingLauncherReExport: vi.fn() }));

    await act(async () => {
      await result.current[handler]();
    });

    const args = findCall('profile_export_toml');
    expect(args).toHaveProperty('includeHooks', includeHooks);
    expect(args).not.toHaveProperty('include_hooks');
  });
});
