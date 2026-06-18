import type { ConfigDiffResult, ConfigSemanticChange } from '../../types/profile-history';

interface SemanticDiffViewProps {
  changes: ConfigSemanticChange[];
  truncated: boolean;
  parseFailed: boolean;
}

/**
 * Compact semantic diff listing field-level TOML changes.
 */
export function SemanticDiffView({ changes, truncated, parseFailed }: SemanticDiffViewProps) {
  if (parseFailed) {
    return (
      <p className="crosshook-help-text" role="status">
        Semantic diff unavailable for this snapshot — showing unified diff instead.
      </p>
    );
  }

  if (changes.length === 0) {
    return (
      <p className="crosshook-help-text" style={{ marginTop: 8 }}>
        No differences found.
      </p>
    );
  }

  return (
    <div>
      {truncated ? (
        <p className="crosshook-help-text" role="status" style={{ marginBottom: 8 }}>
          Showing the first {changes.length} semantic changes — list truncated.
        </p>
      ) : null}
      <ul className="crosshook-history-semantic-list">
        {changes.map((change) => (
          <li key={`${change.path}:${change.change_type}`} className="crosshook-history-semantic-item">
            <span className="crosshook-history-badge">{change.change_type}</span>
            <code className="crosshook-history-semantic-path">{change.path}</code>
            {change.change_type === 'changed' ? (
              <div className="crosshook-help-text" style={{ marginTop: 4 }}>
                <span className="crosshook-history-stat--remove">{change.old_value ?? '—'}</span>
                {' → '}
                <span className="crosshook-history-stat--add">{change.new_value ?? '—'}</span>
              </div>
            ) : change.change_type === 'added' ? (
              <div className="crosshook-help-text crosshook-history-stat--add" style={{ marginTop: 4 }}>
                {change.new_value ?? '—'}
              </div>
            ) : (
              <div className="crosshook-help-text crosshook-history-stat--remove" style={{ marginTop: 4 }}>
                {change.old_value ?? '—'}
              </div>
            )}
          </li>
        ))}
      </ul>
    </div>
  );
}

export function semanticDiffFromResult(diff: ConfigDiffResult): SemanticDiffViewProps {
  return {
    changes: diff.semantic_changes ?? [],
    truncated: diff.truncated,
    parseFailed: diff.semantic_parse_failed ?? false,
  };
}
