import type { FacetKey } from '../../hooks/useCommunityCatalog';
import type { CommunityCompatibilityRating } from '../../hooks/useCommunityProfiles';
import { LOADING_MODE_LABELS } from '../../lib/loadingModes';
import type { CatalogFacets, CatalogFacetValue, CatalogQuery } from '../../types/discovery';
import { ratingLabel, ratingOrder } from '../community/CompatibilityBadge';
import type { SelectOption } from '../ui/ThemedSelect';
import { ThemedSelectField } from '../ui/ThemedSelectField';

const ALL_VALUE = 'all';
const GAME_TITLE_MENU_CAP = 20;
const LOADING_MODE_ORDER = ['source_directory', 'copy_to_prefix', 'unknown'] as const;

export interface DiscoveryFacetBarProps {
  facets: CatalogFacets;
  query: CatalogQuery;
  onSetFacet: (key: FacetKey, value: string | undefined) => void;
  onClearFilters: () => void;
}

function selectedValue(selections: string[] | undefined): string | undefined {
  return selections && selections.length > 0 ? selections[0] : undefined;
}

function shortTapLabel(tapUrl: string): string {
  return (
    tapUrl
      .split('/')
      .filter(Boolean)
      .pop()
      ?.replace(/\.git$/, '') ?? tapUrl
  );
}

function countFor(values: CatalogFacetValue[], value: string): number {
  return values.find((facet) => facet.value === value)?.count ?? 0;
}

/** Fixed-order menus (loading mode / compatibility) always render every value; absent values get count 0 (disabled). */
function fixedOrderOptions(
  order: readonly string[],
  labels: Record<string, string>,
  values: CatalogFacetValue[],
  selected: string | undefined,
  headLabel: string
): SelectOption[] {
  return [
    { value: ALL_VALUE, label: headLabel },
    ...order.map((value) => {
      const count = countFor(values, value);
      return {
        value,
        label: `${labels[value] ?? value} (${count})`,
        disabled: count === 0 && value !== selected,
      };
    }),
  ];
}

interface ActiveFilterChip {
  key: FacetKey;
  dimensionLabel: string;
  displayValue: string;
}

export function DiscoveryFacetBar({ facets, query, onSetFacet, onClearFilters }: DiscoveryFacetBarProps) {
  const selectedTitle = selectedValue(query.gameTitles);
  const selectedMode = selectedValue(query.loadingModes);
  const selectedBand = selectedValue(query.compatibilityBands);
  const selectedTap = selectedValue(query.tapUrls);

  const titleMenu = facets.gameTitles.slice(0, GAME_TITLE_MENU_CAP);
  if (selectedTitle !== undefined && !titleMenu.some((facet) => facet.value === selectedTitle)) {
    titleMenu.push({ value: selectedTitle, count: countFor(facets.gameTitles, selectedTitle) });
  }
  const titleOptions: SelectOption[] = [
    { value: ALL_VALUE, label: 'All games' },
    ...titleMenu.map((facet) => ({
      value: facet.value,
      label: `${facet.value} (${facet.count})`,
      disabled: facet.count === 0 && facet.value !== selectedTitle,
    })),
  ];

  const modeOptions = fixedOrderOptions(
    LOADING_MODE_ORDER,
    LOADING_MODE_LABELS,
    facets.loadingModes,
    selectedMode,
    'All modes'
  );

  const ratingLabels: Record<string, string> = ratingLabel;
  const bandOptions = fixedOrderOptions(
    ratingOrder as readonly CommunityCompatibilityRating[],
    ratingLabels,
    facets.compatibilityBands,
    selectedBand,
    'All ratings'
  );

  const tapOptions: SelectOption[] = [
    { value: ALL_VALUE, label: 'All taps' },
    ...facets.taps.map((facet) => ({
      value: facet.value,
      label: `${shortTapLabel(facet.value)} (${facet.count})`,
      disabled: facet.count === 0 && facet.value !== selectedTap,
    })),
  ];

  const handleSelect = (key: FacetKey) => (value: string) => {
    onSetFacet(key, value === ALL_VALUE ? undefined : value);
  };

  const activeChips: ActiveFilterChip[] = [];
  if (selectedTitle !== undefined) {
    activeChips.push({ key: 'gameTitle', dimensionLabel: 'Game', displayValue: selectedTitle });
  }
  if (selectedMode !== undefined) {
    activeChips.push({
      key: 'loadingMode',
      dimensionLabel: 'Loading mode',
      displayValue: LOADING_MODE_LABELS[selectedMode] ?? selectedMode,
    });
  }
  if (selectedBand !== undefined) {
    activeChips.push({
      key: 'compatibility',
      dimensionLabel: 'Compatibility',
      displayValue: ratingLabels[selectedBand] ?? selectedBand,
    });
  }
  if (selectedTap !== undefined) {
    activeChips.push({ key: 'tap', dimensionLabel: 'Tap', displayValue: shortTapLabel(selectedTap) });
  }

  return (
    <div className="crosshook-discovery-facet-bar">
      <div className="crosshook-discovery-facet-bar__selects">
        <div className="crosshook-discovery-facet-bar__field">
          <ThemedSelectField
            label="Game title"
            value={selectedTitle ?? ALL_VALUE}
            onValueChange={handleSelect('gameTitle')}
            options={titleOptions}
          />
        </div>
        <div className="crosshook-discovery-facet-bar__field">
          <ThemedSelectField
            label="Loading mode"
            value={selectedMode ?? ALL_VALUE}
            onValueChange={handleSelect('loadingMode')}
            options={modeOptions}
          />
        </div>
        <div className="crosshook-discovery-facet-bar__field">
          <ThemedSelectField
            label="Compatibility"
            value={selectedBand ?? ALL_VALUE}
            onValueChange={handleSelect('compatibility')}
            options={bandOptions}
          />
        </div>
        <div className="crosshook-discovery-facet-bar__field" title={selectedTap}>
          <ThemedSelectField
            label="Source tap"
            value={selectedTap ?? ALL_VALUE}
            onValueChange={handleSelect('tap')}
            options={tapOptions}
          />
        </div>
      </div>

      {activeChips.length > 0 && (
        <div className="crosshook-discovery-facet-bar__chips">
          {activeChips.map((chip) => (
            <span key={chip.key} className="crosshook-status-chip">
              {`${chip.dimensionLabel}: ${chip.displayValue}`}
              <button
                type="button"
                className="crosshook-discovery-facet-bar__chip-remove"
                aria-label={`Remove filter ${chip.dimensionLabel} ${chip.displayValue}`}
                onClick={() => onSetFacet(chip.key, undefined)}
              >
                ×
              </button>
            </span>
          ))}
          {activeChips.length >= 2 && (
            <button
              type="button"
              className="crosshook-button crosshook-button--compact crosshook-button--secondary"
              onClick={onClearFilters}
            >
              Clear all
            </button>
          )}
        </div>
      )}
    </div>
  );
}

export default DiscoveryFacetBar;
