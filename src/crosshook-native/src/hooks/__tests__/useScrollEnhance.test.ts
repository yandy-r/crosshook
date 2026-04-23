import { describe, expect, it } from 'vitest';
import { SCROLL_ENHANCE_SELECTORS } from '../useScrollEnhance';

describe('useScrollEnhance selectors', () => {
  it('registers the context rail body scroll target exactly once', () => {
    const matches = SCROLL_ENHANCE_SELECTORS.match(/\.crosshook-context-rail__body\b/g);
    expect(matches?.length).toBe(1);
  });
});
