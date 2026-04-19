import type { LaunchFeedback } from '../../types';
import { sortPatternMatchesBySeverity } from './helpers';

interface LaunchPanelFeedbackProps {
  feedback: LaunchFeedback;
  diagnosticExpanded: boolean;
  setDiagnosticExpanded: (updater: (current: boolean) => boolean) => void;
  diagnosticCopyLabel: string;
  onCopyDiagnosticReport: () => void;
}

export function LaunchPanelFeedback({
  feedback,
  diagnosticExpanded,
  setDiagnosticExpanded,
  diagnosticCopyLabel,
  onCopyDiagnosticReport,
}: LaunchPanelFeedbackProps) {
  const diagnosticFeedback = feedback.kind === 'diagnostic' ? feedback.report : null;
  const validationFeedback = feedback.kind === 'validation' ? feedback.issue : null;
  const runtimeFeedback = feedback.kind === 'runtime' ? feedback.message : null;
  const feedbackSeverity = diagnosticFeedback?.severity ?? validationFeedback?.severity ?? 'fatal';
  const feedbackLabel = feedbackSeverity === 'fatal' ? 'Fatal' : feedbackSeverity === 'warning' ? 'Warning' : 'Info';

  const diagnosticMatches = diagnosticFeedback ? sortPatternMatchesBySeverity(diagnosticFeedback.pattern_matches) : [];
  const visibleDiagnosticMatches = diagnosticExpanded ? diagnosticMatches : diagnosticMatches.slice(0, 3);

  return (
    <div
      className="crosshook-launch-panel__feedback"
      data-kind={feedback.kind}
      data-severity={feedbackSeverity}
      role="alert"
    >
      {diagnosticFeedback ? (
        <>
          <div className="crosshook-launch-panel__feedback-header">
            <span className="crosshook-launch-panel__feedback-badge">{feedbackLabel}</span>
            <p className="crosshook-launch-panel__feedback-title">{diagnosticFeedback.summary}</p>
          </div>
          <p className="crosshook-launch-panel__feedback-help">{diagnosticFeedback.exit_info.description}</p>
          {visibleDiagnosticMatches.length > 0 ? (
            <ul className="crosshook-launch-panel__feedback-list">
              {visibleDiagnosticMatches.map((patternMatch) => (
                <li
                  key={`${diagnosticFeedback.analyzed_at}-${patternMatch.pattern_id}`}
                  className="crosshook-launch-panel__feedback-item"
                >
                  <div className="crosshook-launch-panel__feedback-header">
                    <span className="crosshook-launch-panel__feedback-badge" data-severity={patternMatch.severity}>
                      {patternMatch.severity}
                    </span>
                    <p className="crosshook-launch-panel__feedback-title">{patternMatch.summary}</p>
                  </div>
                  <p className="crosshook-launch-panel__feedback-help">{patternMatch.suggestion}</p>
                </li>
              ))}
            </ul>
          ) : null}
          <div className="crosshook-launch-panel__feedback-actions">
            {diagnosticMatches.length > 3 || diagnosticFeedback.suggestions.length > 0 ? (
              <button
                type="button"
                className="crosshook-button crosshook-button--secondary crosshook-launch-panel__feedback-action"
                onClick={() => setDiagnosticExpanded((current) => !current)}
              >
                {diagnosticExpanded ? 'Show Less' : 'Show Details'}
              </button>
            ) : null}
            <button
              type="button"
              className="crosshook-button crosshook-button--secondary crosshook-launch-panel__feedback-action"
              onClick={onCopyDiagnosticReport}
            >
              {diagnosticCopyLabel}
            </button>
          </div>
          {diagnosticExpanded ? (
            <div className="crosshook-launch-panel__feedback-details">
              <p className="crosshook-launch-panel__feedback-help">
                Exit mode: {diagnosticFeedback.exit_info.failure_mode}
              </p>
              <p className="crosshook-launch-panel__feedback-help">
                Exit code: {diagnosticFeedback.exit_info.code ?? 'n/a'} | Signal:{' '}
                {diagnosticFeedback.exit_info.signal ?? 'n/a'}
              </p>
              {diagnosticFeedback.log_tail_path ? (
                <p className="crosshook-launch-panel__feedback-help">Log tail: {diagnosticFeedback.log_tail_path}</p>
              ) : null}
              {diagnosticFeedback.suggestions.length > 0 ? (
                <ul className="crosshook-launch-panel__feedback-list">
                  {diagnosticFeedback.suggestions.map((suggestion, index) => (
                    <li
                      // biome-ignore lint/suspicious/noArrayIndexKey: tiebreaker when severity+title may collide
                      key={`${suggestion.severity}-${suggestion.title}-${index}`}
                      className="crosshook-launch-panel__feedback-item"
                    >
                      <div className="crosshook-launch-panel__feedback-header">
                        <span className="crosshook-launch-panel__feedback-badge" data-severity={suggestion.severity}>
                          {suggestion.severity}
                        </span>
                        <p className="crosshook-launch-panel__feedback-title">{suggestion.title}</p>
                      </div>
                      <p className="crosshook-launch-panel__feedback-help">{suggestion.description}</p>
                    </li>
                  ))}
                </ul>
              ) : null}
            </div>
          ) : null}
        </>
      ) : validationFeedback ? (
        <>
          <div className="crosshook-launch-panel__feedback-header">
            <span className="crosshook-launch-panel__feedback-badge">{feedbackLabel}</span>
            <p className="crosshook-launch-panel__feedback-title">{validationFeedback.message}</p>
          </div>
          <p className="crosshook-launch-panel__feedback-help">{validationFeedback.help}</p>
        </>
      ) : (
        <p className="crosshook-launch-panel__feedback-title">{runtimeFeedback}</p>
      )}
    </div>
  );
}
