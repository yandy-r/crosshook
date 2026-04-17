import { formatRelativeTime } from '../../utils/format';
import { type HostToolAvailabilityFilter, type HostToolCategoryFilter, HostToolFilterBar } from './HostToolFilterBar';

export interface HostToolStatusToolbarProps {
  lastCheckedAt: string | null;
  isStale: boolean;
  isRefreshing: boolean;
  shownCount: number;
  totalCount: number;
  detectedDistroFamily: string;
  onRefresh: () => void;

  categoryFilter: HostToolCategoryFilter;
  availabilityFilter: HostToolAvailabilityFilter;
  searchQuery: string;
  onCategoryFilterChange: (value: HostToolCategoryFilter) => void;
  onAvailabilityFilterChange: (value: HostToolAvailabilityFilter) => void;
  onSearchQueryChange: (value: string) => void;
  filtersDisabled: boolean;
}

function formatLastChecked(value: string | null): string {
  if (value == null) return 'Never';
  return formatRelativeTime(value);
}

export function HostToolStatusToolbar({
  lastCheckedAt,
  isStale,
  isRefreshing,
  shownCount,
  totalCount,
  detectedDistroFamily,
  onRefresh,
  categoryFilter,
  availabilityFilter,
  searchQuery,
  onCategoryFilterChange,
  onAvailabilityFilterChange,
  onSearchQueryChange,
  filtersDisabled,
}: HostToolStatusToolbarProps) {
  const distroLabel = detectedDistroFamily.trim().length > 0 ? detectedDistroFamily : 'Unknown';

  return (
    <section className="crosshook-panel crosshook-host-tool-dashboard-toolbar" aria-label="Host tool dashboard toolbar">
      <div className="crosshook-host-tool-dashboard-toolbar__status">
        <div className="crosshook-host-tool-dashboard-toolbar__status-row">
          <span className="crosshook-muted">Last checked</span>
          <span className="crosshook-host-tool-dashboard-toolbar__status-value">
            {formatLastChecked(lastCheckedAt)}
          </span>
          {isStale ? (
            <span
              className="crosshook-status-chip crosshook-host-tool-dashboard__status-chip crosshook-host-tool-dashboard__status-chip--degraded"
              title="The cached snapshot is older than 24 hours."
            >
              Stale
            </span>
          ) : null}
        </div>
        <div className="crosshook-host-tool-dashboard-toolbar__status-row">
          <span className="crosshook-muted">Detected host</span>
          <span className="crosshook-host-tool-dashboard-toolbar__status-value">{distroLabel}</span>
        </div>
        <div className="crosshook-host-tool-dashboard-toolbar__status-row">
          <span className="crosshook-muted">Showing</span>
          <span className="crosshook-host-tool-dashboard-toolbar__status-value">
            {shownCount} of {totalCount} {totalCount === 1 ? 'tool' : 'tools'}
          </span>
        </div>
      </div>

      <div className="crosshook-host-tool-dashboard-toolbar__refresh">
        <button
          type="button"
          className="crosshook-button crosshook-button--secondary"
          disabled={isRefreshing}
          aria-busy={isRefreshing}
          onClick={onRefresh}
        >
          {isRefreshing ? 'Refreshing…' : 'Refresh checks'}
        </button>
      </div>

      <div className="crosshook-host-tool-dashboard-toolbar__filters">
        <HostToolFilterBar
          categoryFilter={categoryFilter}
          availabilityFilter={availabilityFilter}
          searchQuery={searchQuery}
          disabled={filtersDisabled}
          onCategoryFilterChange={onCategoryFilterChange}
          onAvailabilityFilterChange={onAvailabilityFilterChange}
          onSearchQueryChange={onSearchQueryChange}
        />
      </div>
    </section>
  );
}

export default HostToolStatusToolbar;
