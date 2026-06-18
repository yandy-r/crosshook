import type { ConfigRevisionSource } from '../../types/profile-history';

/** Human-readable labels for revision sources. */
export const SOURCE_LABELS: Record<ConfigRevisionSource, string> = {
  manual_save: 'Manual save',
  rollback_apply: 'Restore',
  import: 'Import',
  launch_optimization_save: 'Optimization save',
  launch_command_arguments_save: 'Command arguments save',
  preset_apply: 'Preset applied',
  migration: 'Migration',
};

/**
 * Format an ISO date string into a locale-specific readable format.
 * Falls back to the original string if parsing fails.
 */
export function formatExactDate(isoString: string): string {
  try {
    return new Date(isoString).toLocaleString(undefined, {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    });
  } catch {
    return isoString;
  }
}

/** Context lines kept around each change when collapsing unified diff output. */
export const DIFF_CONTEXT_LINES = 3;

/**
 * Collapse unchanged context lines within unified diff hunks, keeping headers and
 * `contextLines` of padding around each added/removed line.
 */
export function collapseUnifiedDiffLines(
  diffText: string,
  showUnchanged: boolean,
  contextLines: number = DIFF_CONTEXT_LINES
): string {
  if (showUnchanged || diffText.trim() === '') {
    return diffText;
  }

  const lines = diffText.split('\n');
  const output: string[] = [];
  let hunkLines: string[] = [];
  let hunkChangedIndices: number[] = [];

  const flushHunk = () => {
    if (hunkLines.length === 0) {
      return;
    }
    if (hunkChangedIndices.length === 0) {
      for (const line of hunkLines) {
        output.push(line);
      }
    } else {
      const keep = new Set<number>();
      for (const idx of hunkChangedIndices) {
        const start = Math.max(0, idx - contextLines);
        const end = Math.min(hunkLines.length - 1, idx + contextLines);
        for (let i = start; i <= end; i += 1) {
          keep.add(i);
        }
      }
      for (let i = 0; i < hunkLines.length; i += 1) {
        const line = hunkLines[i] ?? '';
        if (keep.has(i) || line.startsWith('@@')) {
          output.push(line);
        }
      }
    }
    hunkLines = [];
    hunkChangedIndices = [];
  };

  for (const line of lines) {
    if (line.startsWith('@@')) {
      flushHunk();
      output.push(line);
      hunkLines = [line];
      hunkChangedIndices = [];
      continue;
    }

    if (line.startsWith('---') || line.startsWith('+++')) {
      flushHunk();
      output.push(line);
      continue;
    }

    if (hunkLines.length === 0) {
      hunkLines = [];
    }
    const index = hunkLines.length;
    hunkLines.push(line);
    if ((line.startsWith('+') && !line.startsWith('+++')) || (line.startsWith('-') && !line.startsWith('---'))) {
      hunkChangedIndices.push(index);
    }
  }

  flushHunk();
  return output.join('\n');
}
