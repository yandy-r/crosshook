import type { LaunchValidationIssue } from '@/types/launch';

export interface ModAdvisoryListProps {
  advisories: LaunchValidationIssue[] | null;
  hasEnabledMods: boolean;
}

export function ModAdvisoryList({ advisories, hasEnabledMods }: ModAdvisoryListProps) {
  if (!hasEnabledMods || advisories === null) {
    return null;
  }

  return (
    <div className="crosshook-mods-advisories" role="status">
      {advisories.length === 0 ? (
        <p className="crosshook-mods-advisories__empty">
          No coexistence conflicts detected for the current trainer loading mode.
        </p>
      ) : (
        <ul className="crosshook-mods-advisories__list">
          {advisories.map((issue) => (
            <li key={`${issue.code ?? 'advisory'}:${issue.message}`} className="crosshook-mods-advisories__item">
              <span className="crosshook-launch-panel__feedback-badge" data-severity={issue.severity}>
                {issue.severity}
              </span>
              <div className="crosshook-mods-advisories__text">
                <p className="crosshook-mods-advisories__message">{issue.message}</p>
                <p className="crosshook-mods-advisories__help">{issue.help}</p>
              </div>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
