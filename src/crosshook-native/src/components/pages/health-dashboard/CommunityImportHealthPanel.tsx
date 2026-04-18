import { useMemo } from 'react';
import type { EnrichedProfileHealthReport } from '../../../types/health';
import { HealthBadge } from '../../HealthBadge';
import { CollapsibleSection } from '../../ui/CollapsibleSection';

export function CommunityImportHealthPanel({ profiles }: { profiles: EnrichedProfileHealthReport[] }) {
  const unhealthyImports = useMemo(
    () => profiles.filter((r) => r.metadata?.is_community_import === true && r.status !== 'healthy'),
    [profiles]
  );

  return (
    <CollapsibleSection title="Community Import Health" defaultOpen={false}>
      {unhealthyImports.length === 0 ? (
        <p className="crosshook-muted">All community-imported profiles are healthy.</p>
      ) : (
        <>
          <p className="crosshook-help-text crosshook-muted">
            Imported profiles often need path adjustments for your system.
          </p>
          <ul className="crosshook-health-dashboard-panel-list">
            {unhealthyImports.map((report) => (
              <li key={report.name} className="crosshook-health-dashboard-panel-row">
                <span className="crosshook-health-dashboard-panel-row__name">{report.name}</span>
                <HealthBadge report={report} />
                <span className="crosshook-muted">
                  {report.issues.length} issue{report.issues.length !== 1 ? 's' : ''}
                </span>
              </li>
            ))}
          </ul>
        </>
      )}
    </CollapsibleSection>
  );
}
