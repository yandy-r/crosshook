import CompatibilityViewer, { type CompatibilityDatabaseEntry } from '../CompatibilityViewer';
import { PageBanner, CompatibilityArt } from '../layout/PageBanner';
import { useCommunityProfiles } from '../../hooks/useCommunityProfiles';

const DEFAULT_PROFILES_DIRECTORY = '~/.config/crosshook/profiles';

function toCompatibilityEntries(entries: ReturnType<typeof useCommunityProfiles>['index']['entries']): CompatibilityDatabaseEntry[] {
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
    <>
      <PageBanner
        eyebrow="Community"
        title="Compatibility data"
        copy="Inspect shared compatibility metadata across the current community index and filter it with the viewer below."
        illustration={<CompatibilityArt />}
      />
      <CompatibilityViewer
        entries={compatibilityEntries}
        loading={communityState.loading || communityState.syncing}
        error={communityState.error}
        emptyMessage="No indexed community compatibility entries are available yet."
      />
    </>
  );
}

export default CompatibilityPage;
