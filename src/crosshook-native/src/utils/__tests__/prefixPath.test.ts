import { describe, expect, it } from 'vitest';
import { makeProfileDraft } from '@/test/fixtures';
import { resolveEffectivePrefixPath } from '../prefixPath';

describe('resolveEffectivePrefixPath', () => {
  it('falls back to a trimmed Steam compatdata path when runtime prefix is blank', () => {
    const profile = makeProfileDraft({
      runtime: { prefix_path: '   ' },
      steam: { compatdata_path: '  /steam/compatdata/123/pfx  ' },
    });

    expect(resolveEffectivePrefixPath(profile)).toBe('/steam/compatdata/123/pfx');
  });

  it('prefers a trimmed runtime prefix when both paths exist', () => {
    const profile = makeProfileDraft({
      runtime: { prefix_path: '  /games/pfx  ' },
      steam: { compatdata_path: '/steam/pfx' },
    });

    expect(resolveEffectivePrefixPath(profile)).toBe('/games/pfx');
  });
});
