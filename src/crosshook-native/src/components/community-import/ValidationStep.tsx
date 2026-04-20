import type { LaunchValidationIssue } from '../../types';

interface ValidationStepProps {
  fatalCount: number;
  warningCount: number;
  validationIssues: LaunchValidationIssue[];
  validationError: string | null;
  validating: boolean;
  onValidate: () => void;
}

export function ValidationStep({
  fatalCount,
  warningCount,
  validationIssues,
  validationError,
  validating,
  onValidate,
}: ValidationStepProps) {
  return (
    <div className="crosshook-community-import-wizard__stack">
      <div className="crosshook-community-import-wizard__button-row">
        <button
          type="button"
          className="crosshook-button crosshook-button--secondary"
          onClick={() => onValidate()}
          disabled={validating}
        >
          {validating ? 'Validating...' : 'Re-run Validation'}
        </button>
        <span className="crosshook-muted">
          Fatal: {fatalCount} | Warnings: {warningCount}
        </span>
      </div>
      {validationError ? <p className="crosshook-community-browser__error">{validationError}</p> : null}
      {validationIssues.length > 0 ? (
        <ul className="crosshook-community-import-wizard__validation-list">
          {validationIssues.map((issue) => (
            <li
              key={`${issue.severity}-${issue.code ?? issue.message}`}
              className="crosshook-community-import-wizard__validation-item"
            >
              <strong>[{issue.severity}]</strong> {issue.message}
              {issue.help ? <div className="crosshook-muted">{issue.help}</div> : null}
            </li>
          ))}
        </ul>
      ) : (
        <p className="crosshook-success">No validation issues reported for this draft.</p>
      )}
    </div>
  );
}

export default ValidationStep;
