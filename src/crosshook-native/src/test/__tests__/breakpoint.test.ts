import { describe, expect, it } from 'vitest';
import { BREAKPOINT_PX, breakpointResult } from '../breakpoint';

describe('breakpointResult', () => {
  it('marks deck flags for deck size', () => {
    expect(breakpointResult('deck').isDeck).toBe(true);
    expect(breakpointResult('deck').isUw).toBe(false);
    expect(breakpointResult('deck').width).toBe(BREAKPOINT_PX.deck);
  });

  it('marks uw flags for uw size', () => {
    expect(breakpointResult('uw').isUw).toBe(true);
    expect(breakpointResult('uw').isDeck).toBe(false);
    expect(breakpointResult('uw').width).toBe(BREAKPOINT_PX.uw);
  });
});
