import { renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createDefaultProfile } from '@/types/profile';
import type { CommunityImportResult } from '../useCommunityProfiles';
import { useImportCommunityProfile } from '../useImportCommunityProfile';

const callCommandMock = vi.fn();

vi.mock('@/lib/ipc', () => ({
  callCommand: (name: string, args?: unknown) => callCommandMock(name, args),
}));

describe('useImportCommunityProfile', () => {
  beforeEach(() => {
    callCommandMock.mockReset();
  });

  it('resolves with the backend CommunityImportResult, including the name actually written', async () => {
    const profile = createDefaultProfile();
    const backendResult: CommunityImportResult = {
      profile_name: 'elden-ring-2',
      source_path: '/tmp/tap/elden-ring/community.json',
      profile_path: '/home/user/.config/crosshook/profiles/elden-ring-2.toml',
      profile,
      manifest: {
        schema_version: 1,
        metadata: {
          game_name: 'Elden Ring',
          game_version: '',
          trainer_name: '',
          trainer_version: '',
          proton_version: '',
          platform_tags: [],
          compatibility_rating: 'unknown',
          author: '',
          description: '',
          trainer_sha256: null,
        },
        profile,
      },
    };
    callCommandMock.mockResolvedValue(backendResult);
    const { result } = renderHook(() => useImportCommunityProfile());

    await expect(result.current.importCommunityProfile('/tmp/tap/elden-ring/community.json')).resolves.toBe(
      backendResult
    );
    expect(callCommandMock).toHaveBeenCalledWith('community_import_profile', {
      path: '/tmp/tap/elden-ring/community.json',
    });
  });
});
