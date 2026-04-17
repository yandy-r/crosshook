import { useMemo } from 'react';
import type { Capability, CapabilityState } from '../../types/onboarding';
import CapabilityTile from './CapabilityTile';

export interface CapabilityTilesSectionProps {
  capabilities: Capability[];
  loading: boolean;
}

const STATE_PRIORITY: Record<CapabilityState, number> = {
  unavailable: 0,
  degraded: 1,
  available: 2,
};

function SkeletonTile() {
  return (
    <div className="crosshook-host-tool-dashboard-tile crosshook-host-tool-dashboard-tile--skeleton" aria-hidden="true">
      <div className="crosshook-host-tool-dashboard-skeleton crosshook-host-tool-dashboard-skeleton--label" />
      <div className="crosshook-host-tool-dashboard-skeleton crosshook-host-tool-dashboard-skeleton--state" />
      <div className="crosshook-host-tool-dashboard-skeleton crosshook-host-tool-dashboard-skeleton--meta" />
    </div>
  );
}

export function CapabilityTilesSection({ capabilities, loading }: CapabilityTilesSectionProps) {
  const sorted = useMemo(() => {
    return [...capabilities].sort((a, b) => {
      const priorityDelta = STATE_PRIORITY[a.state] - STATE_PRIORITY[b.state];
      if (priorityDelta !== 0) return priorityDelta;
      return a.label.localeCompare(b.label);
    });
  }, [capabilities]);

  const showSkeleton = loading && capabilities.length === 0;

  if (!showSkeleton && capabilities.length === 0) {
    return null;
  }

  return (
    <section
      className="crosshook-host-tool-dashboard-section"
      aria-labelledby="crosshook-host-tool-dashboard-tiles-heading"
    >
      <h2
        id="crosshook-host-tool-dashboard-tiles-heading"
        className="crosshook-heading-section crosshook-host-tool-dashboard-section__heading"
      >
        Capabilities
      </h2>
      <div className="crosshook-host-tool-dashboard-tiles" aria-busy={showSkeleton}>
        {showSkeleton ? (
          <>
            <SkeletonTile />
            <SkeletonTile />
            <SkeletonTile />
            <SkeletonTile />
          </>
        ) : (
          sorted.map((capability) => <CapabilityTile key={capability.id} capability={capability} />)
        )}
      </div>
    </section>
  );
}

export default CapabilityTilesSection;
