import { describe, expect, it } from 'vitest';
import type { UmuGameIdResolutionSource } from '@/types/launch';
import { umuGameIdResolutionSourceLabel } from '../launchPreviewPresentation';

describe('launchPreviewPresentation', () => {
  it.each<[UmuGameIdResolutionSource, string]>([
    ['explicit_override', 'profile override'],
    ['steam_app_id', 'Steam app id'],
    ['fresh_cache', 'cache hit'],
    ['fresh_lookup', 'fresh HTTP lookup'],
    ['stale_cache', 'stale cache fallback'],
    ['cached_not_found', 'cached not found'],
    ['lookup_disabled', 'lookup disabled'],
    ['missing_hints', 'missing store/codename'],
    ['api_unavailable', 'API unavailable'],
    ['fallback', 'fallback'],
  ])('labels %s as %s', (source, label) => {
    expect(umuGameIdResolutionSourceLabel(source)).toBe(label);
  });
});
