import { beforeEach, describe, expect, it } from 'vitest';
import { makeProfileDraft } from '@/test/fixtures';
import { configureMockHandlers, mockCallCommand } from '@/test/render';
import type { CommunityExportResult } from '../../../hooks/useCommunityProfiles';

// Mock handlers must read the same camelCase top-level invoke keys the
// frontend sends (Tauri maps them to the snake_case Rust parameters).
describe('mock handlers read camelCase invoke keys', () => {
  beforeEach(() => {
    configureMockHandlers();
  });

  it('community_export_profile reads profileName and outputPath', async () => {
    const result = await mockCallCommand<CommunityExportResult>('community_export_profile', {
      profileName: 'Synthetic Quest',
      outputPath: '/tmp/synthetic-quest.json',
    });

    expect(result.profile_name).toBe('Synthetic Quest');
    expect(result.output_path).toBe('/tmp/synthetic-quest.json');
  });

  it('profile_export_toml includes hooks only when includeHooks is true', async () => {
    const data = makeProfileDraft({
      pre_launch_hooks: [{ id: 'h1', name: 'Hook', path: '/tmp/hook.sh', stage: 'pre-launch', enabled: true }],
    });

    const withHooks = await mockCallCommand<string>('profile_export_toml', {
      name: 'Synthetic Quest',
      data,
      includeHooks: true,
    });
    const withoutHooks = await mockCallCommand<string>('profile_export_toml', { name: 'Synthetic Quest', data });

    expect(withHooks).toContain('[[pre_launch_hooks]]');
    expect(withoutHooks).not.toContain('[[pre_launch_hooks]]');
  });
});
