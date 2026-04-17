export interface HostToolDashboardHandoffProps {
  onOpen: () => void;
  description: string;
}

/**
 * Shared onboarding handoff block: opens the dedicated Host Tools route (same target as the sidebar entry).
 */
export function HostToolDashboardHandoff({ onOpen, description }: HostToolDashboardHandoffProps) {
  return (
    <section className="crosshook-panel" aria-label="Host tool dashboard handoff">
      <p className="crosshook-muted crosshook-host-tool-dashboard-handoff__description">{description}</p>
      <button
        type="button"
        className="crosshook-button crosshook-button--secondary crosshook-host-tool-dashboard-handoff__action"
        onClick={onOpen}
      >
        Open Host Tool Dashboard
      </button>
    </section>
  );
}

export default HostToolDashboardHandoff;
