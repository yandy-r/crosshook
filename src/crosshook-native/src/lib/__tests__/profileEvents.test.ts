import { describe, expect, it } from 'vitest';
import type { ProfileChangeReason } from '../../types/profile-events';
import { parseProfilesChanged, profileChangeAffectsHealth } from '../profileEvents';

const ALL_REASONS: ProfileChangeReason[] = [
  { kind: 'saved' },
  { kind: 'deleted' },
  { kind: 'renamed', from: 'Alpha' },
  { kind: 'duplicated' },
  { kind: 'favorite_toggled' },
  { kind: 'rolled_back' },
  { kind: 'preset_applied' },
  { kind: 'imported' },
  { kind: 'migrated' },
];

describe('parseProfilesChanged', () => {
  it.each(ALL_REASONS)('accepts the backend wire shape for $kind', (reason) => {
    const payload = { reason, names: ['Beta'] };

    expect(parseProfilesChanged(payload)).toEqual(payload);
  });

  it('accepts the conservative resync payload', () => {
    expect(parseProfilesChanged({ reason: { kind: 'saved' }, names: [] })).toEqual({
      reason: { kind: 'saved' },
      names: [],
    });
  });

  it('drops unknown fields on the reason', () => {
    expect(parseProfilesChanged({ reason: { kind: 'saved', extra: 1 }, names: ['A'] })).toEqual({
      reason: { kind: 'saved' },
      names: ['A'],
    });
  });

  it.each([
    ['null', null],
    ['legacy string', 'saved'],
    ['legacy name/action object', { name: 'Alpha', action: 'saved' }],
    ['array', []],
    ['missing reason', { names: ['Alpha'] }],
    ['unknown kind', { reason: { kind: 'exploded' }, names: [] }],
    ['inherited key as kind', { reason: { kind: 'toString' }, names: [] }],
    ['non-string kind', { reason: { kind: 1 }, names: [] }],
    ['renamed without from', { reason: { kind: 'renamed' }, names: ['Beta'] }],
    ['renamed with non-string from', { reason: { kind: 'renamed', from: 3 }, names: ['Beta'] }],
    ['missing names', { reason: { kind: 'saved' } }],
    ['non-array names', { reason: { kind: 'saved' }, names: 'Alpha' }],
    ['non-string name', { reason: { kind: 'saved' }, names: ['Alpha', 2] }],
  ])('returns null (resync) for %s', (_label, payload) => {
    expect(parseProfilesChanged(payload)).toBeNull();
  });
});

describe('profileChangeAffectsHealth', () => {
  it.each(ALL_REASONS)('$kind', (reason) => {
    expect(profileChangeAffectsHealth(reason)).toBe(reason.kind !== 'favorite_toggled');
  });
});
