import type { CollectionRow } from '../../../types/collections';
import { ThemedSelect } from '../../ui/ThemedSelect';

const NETWORK_ISOLATION_BADGE = 'No network isolation';
const NETWORK_ISOLATION_BADGE_TITLE =
  'This system cannot enforce network isolation (unshare --net). The profile still launches; traffic is not isolated.';

interface LaunchProfileSelectorProps {
  activeCollection: CollectionRow | null;
  activeCollectionId: string | null;
  filteredProfiles: string[];
  pinnedSet: Set<string>;
  selectedProfile: string;
  showNetworkIsolationBadge: (profileName: string) => boolean;
  onClearCollectionFilter: () => void;
  onSelectProfile: (name: string) => void;
  onTogglePin: (value: string) => void;
}

export function LaunchProfileSelector({
  activeCollection,
  activeCollectionId,
  filteredProfiles,
  pinnedSet,
  selectedProfile,
  showNetworkIsolationBadge,
  onClearCollectionFilter,
  onSelectProfile,
  onTogglePin,
}: LaunchProfileSelectorProps) {
  return (
    <>
      {activeCollection !== null && (
        <div className="crosshook-launch-collection-filter">
          Filtering by: <strong>{activeCollection.name}</strong>
          <button
            type="button"
            className="crosshook-button crosshook-button--ghost crosshook-button--small"
            onClick={onClearCollectionFilter}
            aria-label="Clear collection filter"
          >
            ×
          </button>
        </div>
      )}
      <ThemedSelect
        id="launch-profile-selector"
        value={selectedProfile}
        onValueChange={(name) =>
          // LaunchPage threads `activeCollectionId` so Rust merges the
          // collection's launch defaults via `effective_profile_with`.
          // Editor safety: `ProfilesPage` MUST NOT pass collectionId.
          onSelectProfile(name)
        }
        placeholder="Select a profile"
        pinnedValues={pinnedSet}
        onTogglePin={onTogglePin}
        ariaLabelledby="launch-active-profile-label"
        options={filteredProfiles.map((name) => ({
          value: name,
          label: name,
          badge: showNetworkIsolationBadge(name) ? NETWORK_ISOLATION_BADGE : undefined,
          badgeTitle: showNetworkIsolationBadge(name) ? NETWORK_ISOLATION_BADGE_TITLE : undefined,
        }))}
      />
    </>
  );
}
