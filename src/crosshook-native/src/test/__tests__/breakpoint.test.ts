import { describe, expect, it } from 'vitest';
import { BREAKPOINT_PX, breakpointResult } from '../breakpoint';

describe('breakpointResult', () => {
  it('marks deck flags for deck size', () => {
    expect(breakpointResult('deck').isDeck).toBe(true);
    expect(breakpointResult('deck').isNarrow).toBe(false);
    expect(breakpointResult('deck').isUw).toBe(false);
    expect(breakpointResult('deck').width).toBe(BREAKPOINT_PX.deck);
  });

  it('marks narrow flags for narrow size', () => {
    expect(breakpointResult('narrow').isNarrow).toBe(true);
    expect(breakpointResult('narrow').isDeck).toBe(false);
    expect(breakpointResult('narrow').width).toBe(BREAKPOINT_PX.narrow);
  });

  it('marks desk flags for desk size', () => {
    expect(breakpointResult('desk').isDesk).toBe(true);
    expect(breakpointResult('desk').isDeck).toBe(false);
    expect(breakpointResult('desk').width).toBe(BREAKPOINT_PX.desk);
  });

  it('marks uw flags for uw size', () => {
    expect(breakpointResult('uw').isUw).toBe(true);
    expect(breakpointResult('uw').isDeck).toBe(false);
    expect(breakpointResult('uw').width).toBe(BREAKPOINT_PX.uw);
  });
});
