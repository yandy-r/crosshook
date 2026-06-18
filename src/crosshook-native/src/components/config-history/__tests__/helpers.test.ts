import { describe, expect, it } from 'vitest';
import { collapseUnifiedDiffLines } from '../helpers';

describe('collapseUnifiedDiffLines', () => {
  it('returns full diff when showUnchanged is true', () => {
    const diff = '--- a\n+++ b\n@@ -1,3 +1,3 @@\n context\n-old\n+new\n context2\n';
    expect(collapseUnifiedDiffLines(diff, true)).toBe(diff);
  });

  it('hides unchanged context outside the hunk padding', () => {
    const diff = [
      '--- a',
      '+++ b',
      '@@ -1,10 +1,10 @@',
      'far1',
      'far2',
      'far3',
      'context',
      '-removed',
      '+added',
      'context2',
      'far4',
      'far5',
      'far6',
    ].join('\n');

    const collapsed = collapseUnifiedDiffLines(diff, false, 1);
    expect(collapsed).toContain('-removed');
    expect(collapsed).toContain('+added');
    expect(collapsed).not.toContain('far1');
    expect(collapsed).not.toContain('far6');
  });
});
