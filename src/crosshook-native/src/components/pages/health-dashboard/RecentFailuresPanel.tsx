import type { EnrichedProfileHealthReport } from '../../../types/health';
import { formatRelativeTime } from '../../../utils/format';
import { CollapsibleSection } from '../../ui/CollapsibleSection';

export function RecentFailuresPanel({ profiles }: { profiles: EnrichedProfileHealthReport[] }) {
  if (profiles.length === 0) {
    return (
      <CollapsibleSection title="Recent Failures" defaultOpen={false}>
        <p className="crosshook-muted">No profiles with recent launch failures.</p>
      </CollapsibleSection>
    );
  }
  return (
    <CollapsibleSection title="Recent Failures" defaultOpen={false}>
      <ul className="crosshook-health-dashboard-failures-list">
        {profiles.map((report) => (
          <li key={report.name} className="crosshook-health-dashboard-failures-item">
            <span className="crosshook-health-dashboard-failures-item__name">{report.name}</span>
            <span className="crosshook-status-chip crosshook-health-dashboard-failures-item__count">
              {report.metadata?.failure_count_30d} failure{report.metadata?.failure_count_30d !== 1 ? 's' : ''} (30d)
            </span>
            <span className="crosshook-muted crosshook-health-dashboard-failures-item__last-success">
              {report.metadata?.last_success
                ? `Last success ${formatRelativeTime(report.metadata.last_success)}`
                : 'No successful launches recorded'}
            </span>
          </li>
        ))}
      </ul>
    </CollapsibleSection>
  );
}
