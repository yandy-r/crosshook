import { beforeEach, describe, expect, it } from 'vitest';
import { configureMockHandlers, mockCallCommand } from '@/test/render';
import type { LutrisImportEntry } from '../../../hooks/useLutrisImport';
import type { ProfilesChangedPayload } from '../../../types/profile-events';
import { subscribeEvent } from '../../events';
import { parseProfilesChanged } from '../../profileEvents';

const ALPHA = 'Test Game Alpha';
const BETA = 'Dev Game Beta';

async function captureProfilesChanged(): Promise<unknown[]> {
  const payloads: unknown[] = [];
  await subscribeEvent<unknown>('profiles-changed', (event) => {
    payloads.push(event.payload);
  });
  return payloads;
}

function expectPayloads(actual: unknown[], expected: ProfilesChangedPayload[]): void {
  expect(actual).toEqual(expected);
  for (const payload of actual) {
    expect(parseProfilesChanged(payload)).toEqual(payload);
  }
}

describe('mock profiles-changed producers', () => {
  let payloads: unknown[];

  beforeEach(async () => {
    configureMockHandlers();
    await mockCallCommand('profile_list');
    payloads = await captureProfilesChanged();
  });

  it('favorite toggle emits favorite_toggled with the profile name', async () => {
    await mockCallCommand('profile_set_favorite', { name: ALPHA, favorite: true });
    expectPayloads(payloads, [{ reason: { kind: 'favorite_toggled' }, names: [ALPHA] }]);
  });

  it('rename emits renamed with from and the new name; same-name rename is silent', async () => {
    const hadLauncher = await mockCallCommand<boolean>('profile_rename', { oldName: ALPHA, newName: ALPHA });
    expect(hadLauncher).toBe(false);
    expect(payloads).toEqual([]);

    await mockCallCommand('profile_rename', { oldName: ALPHA, newName: 'Renamed Alpha' });
    expectPayloads(payloads, [{ reason: { kind: 'renamed', from: ALPHA }, names: ['Renamed Alpha'] }]);
  });

  it('duplicate emits duplicated with the copy name; failures are silent', async () => {
    await expect(mockCallCommand('profile_delete', { name: 'Missing Profile' })).rejects.toThrow('[dev-mock]');
    await mockCallCommand('profile_duplicate', { name: ALPHA });
    expectPayloads(payloads, [{ reason: { kind: 'duplicated' }, names: [`${ALPHA} (Copy)`] }]);
  });

  it('ProtonDB accept emits saved', async () => {
    await mockCallCommand('protondb_accept_suggestion', {
      request: { kind: 'env_var', profileName: ALPHA, envKey: 'PROTON_USE_WINED3D', envValue: '0' },
    });
    expectPayloads(payloads, [{ reason: { kind: 'saved' }, names: [ALPHA] }]);
  });

  it('batch migration emits one migrated event; preflight rejection is silent', async () => {
    await expect(
      mockCallCommand('apply_batch_migration', {
        request: { migrations: [{ profile_name: 'Missing Profile', field: 'runtime_proton_path', new_path: '/x' }] },
      })
    ).rejects.toThrow('[dev-mock]');
    expect(payloads).toEqual([]);

    await mockCallCommand('apply_batch_migration', {
      request: {
        migrations: [
          { profile_name: ALPHA, field: 'runtime_proton_path', new_path: '/mock/proton/GE-Proton9-2/proton' },
          { profile_name: BETA, field: 'steam_proton_path', new_path: '/mock/proton/GE-Proton9-2/proton' },
        ],
      },
    });
    expectPayloads(payloads, [{ reason: { kind: 'migrated' }, names: [ALPHA, BETA] }]);
  });

  it('Lutris import emits imported with imported names only; nothing imported is silent', async () => {
    const { entries } = await mockCallCommand<{ entries: LutrisImportEntry[] }>('lutris_prepare_import', {
      directory: null,
    });
    const importable = entries.find((entry) => entry.importable);
    const notImportable = entries.find((entry) => !entry.importable);
    if (!importable || !notImportable) throw new Error('[test-mock] expected mixed Lutris fixture entries');

    await mockCallCommand('lutris_import_profiles', { entries: [notImportable] });
    expect(payloads).toEqual([]);

    await mockCallCommand('lutris_import_profiles', { entries: [importable, notImportable] });
    expectPayloads(payloads, [{ reason: { kind: 'imported' }, names: [importable.suggested_name] }]);
  });

  it('community import emits imported with the imported profile name', async () => {
    const result = await mockCallCommand<{ profile_name: string }>('community_import_profile', {
      path: '/mock/taps/community-profiles/synthetic-quest/community.json',
    });
    expectPayloads(payloads, [{ reason: { kind: 'imported' }, names: [result.profile_name] }]);
  });
});
