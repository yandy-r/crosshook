import CompatibilityViewer, { type CompatibilityDatabaseEntry } from '../CompatibilityViewer';
import { useCommunityProfiles } from '../../hooks/useCommunityProfiles';

const DEFAULT_PROFILES_DIRECTORY = '~/.config/crosshook/profiles';

function toCompatibilityEntries(
  entries: ReturnType<typeof useCommunityProfiles>['index']['entries']
): CompatibilityDatabaseEntry[] {
  return entries.map((entry) => ({
    id: `${entry.tap_url}::${entry.relative_path}`,
    tap_url: entry.tap_url,
    tap_branch: entry.tap_branch,
    manifest_path: entry.manifest_path,
    relative_path: entry.relative_path,
    metadata: entry.manifest.metadata,
  }));
}

export function CompatibilityPage() {
  const communityState = useCommunityProfiles({
    profilesDirectoryPath: DEFAULT_PROFILES_DIRECTORY,
  });
  const compatibilityEntries = toCompatibilityEntries(communityState.index.entries);

  return (
    <div className="crosshook-page-scroll-shell crosshook-page-scroll-shell--fill crosshook-page-scroll-shell--compatibility">
      <div className="crosshook-route-stack crosshook-compatibility-page">
        <div className="crosshook-route-stack__body--fill crosshook-compatibility-page__body">
          <div className="crosshook-route-card-host">
            <div className="crosshook-route-card-scroll">
              <CompatibilityViewer
                entries={compatibilityEntries}
                loading={communityState.loading || communityState.syncing}
                error={communityState.error}
                emptyMessage="No indexed community compatibility entries are available yet."
              />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export default CompatibilityPage;
