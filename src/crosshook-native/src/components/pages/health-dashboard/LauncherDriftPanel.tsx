import { useMemo } from 'react';
import type { EnrichedProfileHealthReport } from '../../../types/health';
import { CollapsibleSection } from '../../ui/CollapsibleSection';
import { DRIFT_STATE_MESSAGES } from './constants';

export function LauncherDriftPanel({ profiles }: { profiles: EnrichedProfileHealthReport[] }) {
  const driftProfiles = useMemo(() => {
    return (profiles ?? []).filter((r) => {
      const state = r.metadata?.launcher_drift_state;
      return state != null && state !== 'aligned' && state !== 'unknown';
    });
  }, [profiles]);

  return (
    <CollapsibleSection title="Launcher Drift" defaultOpen={false}>
      {driftProfiles.length === 0 ? (
        <p className="crosshook-muted">All exported launchers are current.</p>
      ) : (
        <ul className="crosshook-health-dashboard-issues-list">
          {driftProfiles.map((r) => {
            const state = r.metadata?.launcher_drift_state ?? '';
            const message = DRIFT_STATE_MESSAGES[state] ?? state;
            return (
              <li key={r.name} className="crosshook-health-dashboard-issue">
                <span className="crosshook-health-dashboard-issue__field">{r.name}</span>
                <span className="crosshook-health-dashboard-issue__message crosshook-muted">{message}</span>
              </li>
            );
          })}
        </ul>
      )}
    </CollapsibleSection>
  );
}
