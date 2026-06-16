import { describe, expect, it } from 'vitest';
import { createDefaultProfile, normalizeSerializedGameProfile, type SerializedGameProfile } from '@/types/profile';
import { createEmptyProfile } from '../createEmptyProfile';
import { normalizeProfileForEdit, normalizeProfileForSave } from '../profileNormalize';

describe('profile runtime umu hints', () => {
  it('initializes empty profile helpers with umu hint fields', () => {
    expect(createDefaultProfile().runtime).toMatchObject({
      umu_store: '',
      umu_codename: '',
    });
    expect(createEmptyProfile().runtime).toMatchObject({
      umu_store: '',
      umu_codename: '',
    });
  });

  it('normalizes umu store and codename hints for edit/save', () => {
    const profile = createDefaultProfile();
    profile.runtime.umu_store = '  GOG  ';
    profile.runtime.umu_codename = '  Cyberpunk_2077  ';

    const normalized = normalizeProfileForEdit(profile, {}, false);

    expect(normalized.runtime.umu_store).toBe('gog');
    expect(normalized.runtime.umu_codename).toBe('Cyberpunk_2077');
  });

  it('drops umu hints with control characters and caps long values', () => {
    const profile = createDefaultProfile();
    profile.runtime.umu_store = 'go\ng';
    profile.runtime.umu_codename = 'x'.repeat(140);

    const normalized = normalizeProfileForEdit(profile, {}, false);

    expect(normalized.runtime.umu_store).toBe('');
    expect(normalized.runtime.umu_codename).toHaveLength(128);
  });
});

describe('profile launch command arguments', () => {
  it('initializes default profiles with empty command arguments', () => {
    expect(createDefaultProfile().launch.command_arguments).toEqual({
      enabled_argument_ids: [],
      custom_args: [],
    });
    expect(normalizeProfileForEdit(createEmptyProfile(), {}, false).launch.command_arguments).toEqual({
      enabled_argument_ids: [],
      custom_args: [],
    });
  });

  it('normalizes older serialized profiles missing command_arguments', () => {
    const legacyProfile = {
      game: { name: 'Legacy Game', executable_path: '/games/legacy/game.exe' },
      trainer: { path: '', type: '', loading_mode: 'source_directory' as const },
      injection: {
        dll_paths: [],
        inject_on_launch: [],
        loaded_hooks: [],
        method: 'disabled' as const,
        stage: 'trainer_launch' as const,
        timeout_ms: 0,
        fallback: 'warn_and_continue' as const,
      },
      steam: {
        enabled: false,
        app_id: '',
        compatdata_path: '',
        proton_path: '',
        launcher: { icon_path: '', display_name: '' },
      },
      launch: {
        method: 'proton_run' as const,
        optimizations: { enabled_option_ids: [] },
        custom_env_vars: {},
      },
    } as SerializedGameProfile;

    const normalized = normalizeSerializedGameProfile(legacyProfile);

    expect(normalized.launch.command_arguments).toEqual({
      enabled_argument_ids: [],
      custom_args: [],
    });
  });

  it('trims curated argument IDs and deduplicates for edit', () => {
    const profile = createDefaultProfile();
    profile.launch.command_arguments = {
      enabled_argument_ids: ['  force_vulkan  ', 'force_vulkan', ''],
      custom_args: ['  --dx11  '],
    };

    const normalized = normalizeProfileForEdit(profile, {}, false);

    expect(normalized.launch.command_arguments.enabled_argument_ids).toEqual(['force_vulkan']);
    expect(normalized.launch.command_arguments.custom_args).toEqual(['--dx11']);
  });

  it('preserves non-blank custom args with control characters during edit', () => {
    const profile = createDefaultProfile();
    profile.launch.command_arguments = {
      enabled_argument_ids: [],
      custom_args: ['--bad\x00arg', 'valid-token'],
    };

    const normalized = normalizeProfileForEdit(profile, {}, false);

    expect(normalized.launch.command_arguments.custom_args).toEqual(['--bad\x00arg', 'valid-token']);
  });

  it('drops blank custom rows only when normalizing for save', () => {
    const profile = createDefaultProfile();
    profile.launch.command_arguments = {
      enabled_argument_ids: ['  force_vulkan  '],
      custom_args: ['   ', '--dx11', '\t', '--bad\x00arg'],
    };

    const forEdit = normalizeProfileForEdit(profile, {}, false);
    expect(forEdit.launch.command_arguments.custom_args).toEqual(['', '--dx11', '', '--bad\x00arg']);

    const forSave = normalizeProfileForSave(profile, {}, false);
    expect(forSave.launch.command_arguments).toEqual({
      enabled_argument_ids: ['force_vulkan'],
      custom_args: ['--dx11', '--bad\x00arg'],
    });
  });

  it('round-trips populated command arguments through edit normalization', () => {
    const profile = createDefaultProfile();
    profile.launch.command_arguments = {
      enabled_argument_ids: ['force_vulkan', 'skip_launcher'],
      custom_args: ['-- %command%', '+set sv_cheats 1'],
    };

    const normalized = normalizeProfileForEdit(profile, {}, false);

    expect(normalized.launch.command_arguments).toEqual({
      enabled_argument_ids: ['force_vulkan', 'skip_launcher'],
      custom_args: ['-- %command%', '+set sv_cheats 1'],
    });
  });
});
