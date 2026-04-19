import type { RefObject } from 'react';

import { formatRelativeTime } from '../../../utils/format';
import { CollapsibleSection } from '../../ui/CollapsibleSection';

interface ProfileHealthIssue {
  field: string;
  message: string;
  path?: string | null;
  remediation?: string | null;
  severity: string;
}

interface ProfileHealthMetadata {
  failure_count_30d: number;
  is_community_import: boolean;
  last_success: string | null;
  launcher_drift_state: string | null;
  total_launches: number;
}

interface ProfileHealthReport {
  issues: ProfileHealthIssue[];
  metadata?: ProfileHealthMetadata | null;
  name: string;
  status: string;
}

interface ProfilesHealthIssuesProps {
  healthIssuesRef?: RefObject<HTMLDivElement>;
  report?: ProfileHealthReport;
}

const fallbackRef = { current: null } as RefObject<HTMLDivElement>;

export function ProfilesHealthIssues({ healthIssuesRef = fallbackRef, report }: ProfilesHealthIssuesProps) {
  if (!report || (report.status !== 'broken' && report.status !== 'stale') || report.issues.length === 0) {
    return null;
  }

  const metadata = report.metadata ?? null;
  const driftMessage: Record<string, string> = {
    missing: 'Exported launcher not found — re-export recommended',
    moved: 'Exported launcher has moved — re-export recommended',
    stale: 'Exported launcher may be outdated — re-export recommended',
  };
  const driftWarning =
    metadata !== null && metadata.launcher_drift_state !== null
      ? (driftMessage[metadata.launcher_drift_state] ?? null)
      : null;

  return (
    <div ref={healthIssuesRef}>
      <CollapsibleSection title="Health Issues" className="crosshook-panel">
        {metadata !== null ? (
          <div style={{ marginBottom: 10, display: 'grid', gap: 4 }}>
            {metadata.last_success !== null ? (
              <p className="crosshook-help-text" style={{ margin: 0 }}>
                Last worked: {formatRelativeTime(metadata.last_success)}
              </p>
            ) : null}
            {metadata.total_launches > 0 ? (
              <p className="crosshook-help-text" style={{ margin: 0 }}>
                Launched {metadata.total_launches} time{metadata.total_launches !== 1 ? 's' : ''} &bull;{' '}
                {metadata.failure_count_30d} failure{metadata.failure_count_30d !== 1 ? 's' : ''} in last 30 days
              </p>
            ) : null}
            {driftWarning !== null ? (
              <p className="crosshook-danger" style={{ margin: 0 }} role="alert">
                {driftWarning}
              </p>
            ) : null}
            {metadata.is_community_import && (report.status === 'broken' || report.status === 'stale') ? (
              <p className="crosshook-help-text" style={{ margin: 0 }}>
                This profile was imported from a community tap — paths may need adjustment for your system.
              </p>
            ) : null}
          </div>
        ) : null}
        <ul style={{ margin: 0, padding: 0, listStyle: 'none', display: 'grid', gap: 8 }}>
          {report.issues.map((issue) => (
            <li
              key={`${report.name}-${issue.field}-${issue.path}-${issue.message}-${issue.severity}`}
              style={{ borderLeft: '3px solid var(--crosshook-danger, #ef4444)', paddingLeft: 10 }}
            >
              <strong>{issue.field}</strong>
              {issue.path ? <span className="crosshook-muted"> — {issue.path}</span> : null}
              <p style={{ margin: '2px 0' }}>{issue.message}</p>
              {issue.remediation ? (
                <p className="crosshook-help-text" style={{ margin: '2px 0' }}>
                  {issue.remediation}
                </p>
              ) : null}
            </li>
          ))}
        </ul>
      </CollapsibleSection>
    </div>
  );
}
