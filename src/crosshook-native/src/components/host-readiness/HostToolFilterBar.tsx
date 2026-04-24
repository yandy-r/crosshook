import { type ChangeEvent, useId } from 'react';

import { type SelectOption, ThemedSelect } from '../ui/ThemedSelect';

export type HostToolCategoryFilter = 'all' | 'runtime' | 'performance' | 'overlay' | 'compatibility' | 'prefix_tools';

export type HostToolAvailabilityFilter = 'all' | 'available' | 'missing' | 'required_missing';

const HOST_TOOL_CATEGORY_FILTER_VALUES: readonly HostToolCategoryFilter[] = [
  'all',
  'runtime',
  'performance',
  'overlay',
  'compatibility',
  'prefix_tools',
];

const HOST_TOOL_AVAILABILITY_FILTER_VALUES: readonly HostToolAvailabilityFilter[] = [
  'all',
  'available',
  'missing',
  'required_missing',
];

export const HOST_TOOL_CATEGORY_FILTER_OPTIONS = [
  { value: 'all', label: 'All categories' },
  { value: 'runtime', label: 'Runtime' },
  { value: 'performance', label: 'Performance' },
  { value: 'overlay', label: 'Overlay' },
  { value: 'compatibility', label: 'Compatibility' },
  { value: 'prefix_tools', label: 'Prefix tools' },
] as const satisfies ReadonlyArray<SelectOption>;

export const HOST_TOOL_AVAILABILITY_FILTER_OPTIONS = [
  { value: 'all', label: 'All availability' },
  { value: 'available', label: 'Available' },
  { value: 'missing', label: 'Missing' },
  { value: 'required_missing', label: 'Required missing' },
] as const satisfies ReadonlyArray<SelectOption>;

export interface HostToolFilterBarProps {
  categoryFilter: HostToolCategoryFilter;
  availabilityFilter: HostToolAvailabilityFilter;
  searchQuery: string;
  onCategoryFilterChange: (value: HostToolCategoryFilter) => void;
  onAvailabilityFilterChange: (value: HostToolAvailabilityFilter) => void;
  onSearchQueryChange: (value: string) => void;
  disabled?: boolean;
}

function toHostToolCategoryFilter(value: string): HostToolCategoryFilter {
  if (HOST_TOOL_CATEGORY_FILTER_VALUES.includes(value as HostToolCategoryFilter)) {
    return value as HostToolCategoryFilter;
  }

  throw new Error(`Unsupported host tool category filter: ${value}`);
}

function toHostToolAvailabilityFilter(value: string): HostToolAvailabilityFilter {
  if (HOST_TOOL_AVAILABILITY_FILTER_VALUES.includes(value as HostToolAvailabilityFilter)) {
    return value as HostToolAvailabilityFilter;
  }

  throw new Error(`Unsupported host tool availability filter: ${value}`);
}

export function HostToolFilterBar({
  categoryFilter,
  availabilityFilter,
  searchQuery,
  onCategoryFilterChange,
  onAvailabilityFilterChange,
  onSearchQueryChange,
  disabled = false,
}: HostToolFilterBarProps) {
  const categoryFieldId = useId();
  const categoryLabelId = useId();
  const availabilityFieldId = useId();
  const availabilityLabelId = useId();
  const searchFieldId = useId();
  const hasActiveFilters = categoryFilter !== 'all' || availabilityFilter !== 'all' || searchQuery.trim().length > 0;

  const handleSearchChange = (event: ChangeEvent<HTMLInputElement>) => {
    onSearchQueryChange(event.target.value);
  };

  const handleClearFilters = () => {
    onCategoryFilterChange('all');
    onAvailabilityFilterChange('all');
    onSearchQueryChange('');
  };

  return (
    <section className="crosshook-host-tool-dashboard__filters" aria-label="Host tool dashboard filters">
      <div className="crosshook-host-tool-dashboard__filters-group" style={{ flex: '1 1 720px' }}>
        <div className="crosshook-field" style={{ flex: '1 1 220px', minWidth: 200 }}>
          <label id={categoryLabelId} className="crosshook-label" htmlFor={categoryFieldId}>
            Category
          </label>
          <ThemedSelect
            id={categoryFieldId}
            ariaLabelledby={categoryLabelId}
            value={categoryFilter}
            options={[...HOST_TOOL_CATEGORY_FILTER_OPTIONS]}
            disabled={disabled}
            onValueChange={(value) => onCategoryFilterChange(toHostToolCategoryFilter(value))}
          />
        </div>

        <div className="crosshook-field" style={{ flex: '1 1 220px', minWidth: 200 }}>
          <label id={availabilityLabelId} className="crosshook-label" htmlFor={availabilityFieldId}>
            Availability
          </label>
          <ThemedSelect
            id={availabilityFieldId}
            ariaLabelledby={availabilityLabelId}
            value={availabilityFilter}
            options={[...HOST_TOOL_AVAILABILITY_FILTER_OPTIONS]}
            disabled={disabled}
            onValueChange={(value) => onAvailabilityFilterChange(toHostToolAvailabilityFilter(value))}
          />
        </div>

        <label className="crosshook-field" htmlFor={searchFieldId} style={{ flex: '2 1 320px', minWidth: 240 }}>
          <span className="crosshook-label">Search</span>
          <input
            id={searchFieldId}
            type="search"
            className="crosshook-input"
            value={searchQuery}
            disabled={disabled}
            maxLength={200}
            autoComplete="off"
            placeholder="Search host tools"
            aria-label="Search host tools"
            onChange={handleSearchChange}
          />
        </label>
      </div>

      <div className="crosshook-host-tool-dashboard__filters-group">
        <button
          type="button"
          className="crosshook-button crosshook-button--ghost"
          disabled={disabled || !hasActiveFilters}
          onClick={handleClearFilters}
        >
          Clear filters
        </button>
      </div>
    </section>
  );
}

export default HostToolFilterBar;
