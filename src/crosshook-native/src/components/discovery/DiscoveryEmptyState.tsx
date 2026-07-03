export interface DiscoveryEmptyStateProps {
  variant: 'no-taps' | 'no-matches';
  onNavigateToBrowse?: () => void;
}

export function DiscoveryEmptyState({ variant, onNavigateToBrowse }: DiscoveryEmptyStateProps) {
  if (variant === 'no-taps') {
    return (
      <div className="crosshook-discovery-panel__empty">
        <p className="crosshook-muted">No community taps are synced yet.</p>
        <p className="crosshook-muted crosshook-discovery-panel__empty-hint">
          Check the online results below, or add community taps with <code>trainer-sources.json</code> manifests for
          local discovery.
        </p>
        {onNavigateToBrowse ? (
          <button type="button" className="crosshook-button crosshook-button--secondary" onClick={onNavigateToBrowse}>
            Manage taps in Browse
          </button>
        ) : null}
      </div>
    );
  }

  return (
    <div className="crosshook-discovery-panel__empty">
      <p className="crosshook-muted">No trainers match the current filters.</p>
      <p className="crosshook-muted crosshook-discovery-panel__empty-hint">
        Clear the search or remove active filters to widen the catalog.
      </p>
    </div>
  );
}

export default DiscoveryEmptyState;
