import type { ConfigDiffResult } from '../../types/profile-history';

interface DiffViewProps {
  diff: ConfigDiffResult;
}

/**
 * Renders a unified diff view with add/remove line statistics.
 * Shows a truncation notice when profiles exceed the 2,000 line limit.
 */
export function DiffView({ diff }: DiffViewProps) {
  if (diff.diff_text.trim() === '') {
    return (
      <p className="crosshook-help-text" style={{ marginTop: 8 }}>
        No differences found.
      </p>
    );
  }

  const lines = diff.diff_text.split('\n');

  return (
    <div>
      <div className="crosshook-history-diff-stats">
        <span className="crosshook-history-stat--add">+{diff.added_lines} added</span>
        <span className="crosshook-history-stat--remove">-{diff.removed_lines} removed</span>
        {diff.truncated && (
          <span className="crosshook-help-text" style={{ marginLeft: 8 }}>
            (truncated — profile exceeds 2 000 lines)
          </span>
        )}
      </div>
      <section aria-label="Unified diff">
        <pre className="crosshook-history-diff-code">
          {lines.map((line, idx) => {
            let cls = 'crosshook-history-diff-line';
            if (line.startsWith('+') && !line.startsWith('+++')) {
              cls += ' crosshook-history-diff-line--add';
            } else if (line.startsWith('-') && !line.startsWith('---')) {
              cls += ' crosshook-history-diff-line--remove';
            } else if (line.startsWith('@@')) {
              cls += ' crosshook-history-diff-line--meta';
            }
            return (
              // biome-ignore lint/suspicious/noArrayIndexKey: lines from diff_text.split('\n') have stable order and no unique identity
              <span key={idx} className={cls}>
                {line}
                {'\n'}
              </span>
            );
          })}
        </pre>
      </section>
    </div>
  );
}
