import type { HostToolCheckResult } from '../../types/onboarding';
import HostToolCard from './HostToolCard';

type SectionTone = 'muted' | 'available' | 'degraded' | 'unavailable';

export interface HostToolInventoryProps {
  requiredTools: HostToolCheckResult[];
  optionalTools: HostToolCheckResult[];
  probingToolId: string | null;
  onProbeDetails: (toolId: string) => Promise<void>;
  onDismissReadinessNag: (toolId: string) => void;
}

function getRequiredTone(tools: HostToolCheckResult[]): SectionTone {
  if (tools.length === 0) return 'muted';
  return tools.every((t) => t.is_available) ? 'available' : 'unavailable';
}

function getOptionalTone(tools: HostToolCheckResult[]): SectionTone {
  if (tools.length === 0) return 'muted';
  if (tools.every((t) => t.is_available)) return 'available';
  return tools.some((t) => t.is_available) ? 'degraded' : 'unavailable';
}

interface InventoryGroupProps {
  title: string;
  description: string;
  tone: SectionTone;
  tools: HostToolCheckResult[];
  emptyMessage: string;
  ariaLabel: string;
  probingToolId: string | null;
  onProbeDetails: (toolId: string) => Promise<void>;
  onDismissReadinessNag: (toolId: string) => void;
}

function InventoryGroup({
  title,
  description,
  tone,
  tools,
  emptyMessage,
  ariaLabel,
  probingToolId,
  onProbeDetails,
  onDismissReadinessNag,
}: InventoryGroupProps) {
  return (
    <section
      className={`crosshook-host-tool-dashboard__card-shell crosshook-host-tool-dashboard__card-shell--${tone}`}
      aria-label={ariaLabel}
    >
      <header className="crosshook-host-tool-dashboard__card-header">
        <div className="crosshook-host-tool-dashboard__card-title-group">
          <h2 className="crosshook-host-tool-dashboard__card-title">{title}</h2>
          <p className="crosshook-host-tool-dashboard__card-summary">{description}</p>
        </div>
        <span className="crosshook-status-chip crosshook-status-chip--muted">{tools.length} shown</span>
      </header>

      <div className="crosshook-host-tool-dashboard__card-body">
        {tools.length > 0 ? (
          <div className="crosshook-host-tool-dashboard__grid">
            {tools.map((tool) => (
              <HostToolCard
                key={tool.tool_id}
                tool={tool}
                isProbingDetails={probingToolId === tool.tool_id}
                onProbeDetails={onProbeDetails}
                onDismissReadinessNag={onDismissReadinessNag}
              />
            ))}
          </div>
        ) : (
          <p className="crosshook-host-tool-dashboard__card-summary">{emptyMessage}</p>
        )}
      </div>
    </section>
  );
}

export function HostToolInventory({
  requiredTools,
  optionalTools,
  probingToolId,
  onProbeDetails,
  onDismissReadinessNag,
}: HostToolInventoryProps) {
  return (
    <>
      <InventoryGroup
        title="Required tools"
        description="These host tools gate core launch and runtime workflows."
        tone={getRequiredTone(requiredTools)}
        tools={requiredTools}
        emptyMessage="No required tools match the active filters."
        ariaLabel="Required host tools"
        probingToolId={probingToolId}
        onProbeDetails={onProbeDetails}
        onDismissReadinessNag={onDismissReadinessNag}
      />

      <InventoryGroup
        title="Optional tools"
        description="These integrations improve capability coverage but do not block baseline launches."
        tone={getOptionalTone(optionalTools)}
        tools={optionalTools}
        emptyMessage="No optional tools match the active filters."
        ariaLabel="Optional host tools"
        probingToolId={probingToolId}
        onProbeDetails={onProbeDetails}
        onDismissReadinessNag={onDismissReadinessNag}
      />
    </>
  );
}

export default HostToolInventory;
