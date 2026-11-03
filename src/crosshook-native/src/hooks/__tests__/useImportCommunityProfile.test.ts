import { renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { makeCommunityImportResult } from '@/test/fixtures';
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
    const backendResult = makeCommunityImportResult({
      profile_name: 'elden-ring-2',
      profile_path: '/home/user/.config/crosshook/profiles/elden-ring-2.toml',
    });
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
