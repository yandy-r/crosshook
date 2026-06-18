import { describe, expect, it } from 'vitest';
import { createEmptyProfile } from '../createEmptyProfile';
import { mergeLiveLaunchSections } from '../useProfileCrud';

describe('mergeLiveLaunchSections', () => {
  it('preserves live command_arguments when a stale draft omits a recent toggle', () => {
    const staleDraft = createEmptyProfile();
    staleDraft.launch.command_arguments = { enabled_argument_ids: [], custom_args: [] };

    const liveProfile = createEmptyProfile();
    liveProfile.launch.command_arguments = {
      enabled_argument_ids: ['skip_launcher'],
      custom_args: ['--custom-flag'],
    };

    const merged = mergeLiveLaunchSections(staleDraft, liveProfile);

    expect(merged.launch.command_arguments).toEqual({
      enabled_argument_ids: ['skip_launcher'],
      custom_args: ['--custom-flag'],
    });
  });

  it('keeps draft-specific launch fields such as custom_env_vars', () => {
    const staleDraft = createEmptyProfile();
    staleDraft.launch.custom_env_vars = { FOO: 'BAR' };

    const liveProfile = createEmptyProfile();
    liveProfile.launch.command_arguments = {
      enabled_argument_ids: ['force_vulkan'],
      custom_args: [],
    };

    const merged = mergeLiveLaunchSections(staleDraft, liveProfile);

    expect(merged.launch.custom_env_vars).toEqual({ FOO: 'BAR' });
    expect(merged.launch.command_arguments.enabled_argument_ids).toEqual(['force_vulkan']);
  });
});
